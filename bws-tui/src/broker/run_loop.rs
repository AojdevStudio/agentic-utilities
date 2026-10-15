//! Daemon run steps 6 to 8 from ADR 0002: start the child, move bytes, mask
//! output, forward stdin and signals, and report the exit status.
//!
//! The supervisor never blocks on the client for longer than one tick, so a
//! forwarded signal or a disconnect is handled while output waits. When the
//! run ends, the child and every process left in its original group are
//! killed, every worker is joined, and only then is the child reaped.

use super::mask::Masker;
use super::protocol::{write_frame, Frame, ResponseFrame};
use super::run_io::{
    spawn_client_reader, spawn_delivery, spawn_reader, spawn_waiter, Cancel, Control, Output,
    STDIN_WINDOW,
};
use super::run_output::Outbox;
use super::run_prepare::PreparedRun;
use super::runspec::{RunOutcome, StreamKind};
use super::spawn::spawn;
use anyhow::Result;
use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Descendants get this long after the child exits before the group is
/// killed. Readers stop after twice this much time in which the client kept
/// up, so a slow client never loses output that was already written.
const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(20);

pub(crate) fn serve_run(mut stream: UnixStream, prepared: PreparedRun) -> Result<()> {
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    let PreparedRun {
        plan,
        fd_values,
        mask_values,
        short_keys,
    } = prepared;
    let client_stream = stream.try_clone()?;
    let child = spawn(&plan);
    drop(plan);
    let child = match child {
        Ok(child) => child,
        Err(error) => {
            let frame = ResponseFrame::Error {
                code: "run_failed".into(),
                message: error.to_string(),
            };
            write_frame(&mut stream, &Frame::Response(frame))?;
            return Ok(());
        }
    };
    let cancel = Cancel::default();
    let mut supervisor = Supervisor {
        pid: child.pid,
        cancel: Cancel::clone(&cancel),
        out: Outbox::default(),
        controls: short_keys
            .into_iter()
            .map(|key| ResponseFrame::RunNotice {
                message: format!(
                    "hush: {key} is shorter than 8 bytes, so it is not masked in this run's output"
                ),
            })
            .collect(),
        maskers: [
            Masker::new(mask_values.iter().cloned()),
            Masker::new(mask_values.iter().cloned()),
        ],
        open: [true, true],
        exited: None,
        exit_queued: false,
        credit: STDIN_WINDOW,
        unpressured: Duration::ZERO,
        last_tick: Instant::now(),
        group_killed: false,
    };
    drop(mask_values);
    let mut workers = Vec::new();
    for (pipe, value) in child.delivery.into_iter().zip(fd_values) {
        workers.push(spawn_delivery(pipe, value, Cancel::clone(&cancel)));
    }
    let (output_tx, output) = sync_channel(16);
    let (control_tx, control) = sync_channel(256);
    workers.push(spawn_reader(
        child.stdout,
        StreamKind::Stdout,
        output_tx.clone(),
        Cancel::clone(&cancel),
    ));
    workers.push(spawn_reader(
        child.stderr,
        StreamKind::Stderr,
        output_tx,
        Cancel::clone(&cancel),
    ));
    workers.push(spawn_waiter(child.pid, control_tx.clone()));
    workers.extend(spawn_client_reader(
        client_stream,
        child.stdin,
        control_tx,
        Cancel::clone(&cancel),
    ));
    let result = supervisor.supervise(&mut stream, &output, &control);
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    kill_group(child.pid, libc::SIGKILL);
    // The child can leave its original group, so it is also killed by pid.
    // SAFETY: the waiter observed the child only with WNOWAIT, so the pid
    // stays reserved until the reap below and names no other process.
    unsafe { libc::kill(child.pid, libc::SIGKILL) };
    // Shutdown ends the client reader; dropping the receivers frees any blocked send.
    let _ = stream.shutdown(std::net::Shutdown::Both);
    drop((output, control, supervisor));
    for worker in workers {
        let _ = worker.join();
    }
    let mut status = 0;
    // SAFETY: the child is ours and the waiter only observed it with WNOWAIT.
    while unsafe { libc::waitpid(child.pid, &mut status, 0) } < 0
        && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
    {}
    result
}

struct Supervisor {
    pid: libc::pid_t,
    cancel: Cancel,
    out: Outbox,
    /// Notices not yet queued, sent one at a time ahead of output.
    controls: VecDeque<ResponseFrame>,
    maskers: [Masker; 2],
    open: [bool; 2],
    /// The exit status and when the child exited.
    exited: Option<(RunOutcome, Instant)>,
    exit_queued: bool,
    /// Stdin credit not yet granted to the client.
    credit: usize,
    /// Time since exit in which the client kept up with output.
    unpressured: Duration,
    last_tick: Instant,
    group_killed: bool,
}

impl Supervisor {
    /// Returns when the run is over. An error means the client cannot get a
    /// complete reply; the caller still cleans up.
    fn supervise(
        &mut self,
        stream: &mut UnixStream,
        output: &Receiver<Output>,
        control: &Receiver<Control>,
    ) -> Result<()> {
        loop {
            while let Ok(event) = control.try_recv() {
                if !self.handle(event) {
                    return Ok(());
                }
            }
            self.offer_controls()?;
            let now = Instant::now();
            if self.exited.is_some() && self.out.accepts_output() {
                self.unpressured += now - self.last_tick;
            }
            self.last_tick = now;
            if let Some((outcome, exited_at)) = self.exited {
                if self.open == [false, false] {
                    if !self.exit_queued && self.controls.is_empty() && !self.out.release_waiting()
                    {
                        self.exit_queued = self.out.offer(&ResponseFrame::RunExit { outcome })?;
                    }
                } else if !self.group_killed && now - exited_at >= DRAIN_AFTER_EXIT {
                    kill_group(self.pid, libc::SIGKILL);
                    self.group_killed = true;
                } else if self.group_killed && self.unpressured >= 2 * DRAIN_AFTER_EXIT {
                    // A process outside the group still holds a pipe.
                    self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
            if !self.out.send(stream)? {
                return Ok(());
            }
            if self.exit_queued && self.out.is_drained() {
                return Ok(());
            }
            if self.open != [false, false] && self.out.accepts_output() {
                match output.recv_timeout(TICK) {
                    Ok(Output::Chunk(kind, chunk)) => {
                        let masked = self.maskers[index(kind)].push(&chunk);
                        self.out.release(kind, masked)?;
                    }
                    Ok(Output::Eof(kind)) => self.close_stream(kind)?,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => self.open = [false, false],
                }
            } else if !self.out.is_drained() {
                self.out.wait_writable(stream, TICK);
            } else {
                match control.recv_timeout(TICK) {
                    Ok(event) => {
                        if !self.handle(event) {
                            return Ok(());
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => std::thread::sleep(TICK),
                }
            }
        }
    }

    /// Queue waiting notices, then any stdin credit, as far as the queue has room.
    fn offer_controls(&mut self) -> Result<()> {
        while let Some(notice) = self.controls.front() {
            if !self.out.offer(notice)? {
                return Ok(());
            }
            self.controls.pop_front();
        }
        let credit = ResponseFrame::RunStdinCredit {
            bytes: self.credit as u32,
        };
        if self.credit > 0 && self.out.offer(&credit)? {
            self.credit = 0;
        }
        Ok(())
    }

    /// False when the client is gone.
    fn handle(&mut self, event: Control) -> bool {
        match event {
            Control::Exited(outcome) => {
                self.exited = Some((outcome, Instant::now()));
                self.last_tick = Instant::now();
            }
            Control::Signal(signal) => kill_group(self.pid, signal.number()),
            Control::StdinConsumed(bytes) => self.credit += bytes,
            Control::ClientGone => return false,
        }
        true
    }

    fn close_stream(&mut self, kind: StreamKind) -> Result<()> {
        if self.open[index(kind)] {
            self.open[index(kind)] = false;
            let tail = self.maskers[index(kind)].finish();
            self.out.release(kind, tail)?;
        }
        Ok(())
    }
}

fn index(kind: StreamKind) -> usize {
    match kind {
        StreamKind::Stdout => 0,
        StreamKind::Stderr => 1,
    }
}

fn kill_group(pid: libc::pid_t, signal: i32) {
    // SAFETY: the child started its own process group, and its pid stays
    // reserved until the final reap, so -pid targets only that group.
    unsafe { libc::kill(-pid, signal) };
}
