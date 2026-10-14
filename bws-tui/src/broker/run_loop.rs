//! Daemon run steps 6 to 8 from ADR 0002: start the child, move bytes, mask
//! output, forward stdin and signals, and report the exit status.
//!
//! The supervisor never blocks on the client for longer than one tick, so a
//! forwarded signal or a disconnect is handled while output waits. When the
//! run ends, every process left in the child's group is killed, every worker
//! is joined, and only then is the child reaped.

use super::mask::Masker;
use super::protocol::{write_frame, Frame, ResponseFrame, WireBytes};
use super::run_io::{
    spawn_client_reader, spawn_delivery, spawn_reader, spawn_waiter, Cancel, Control, Output,
    STDIN_WINDOW,
};
use super::run_prepare::PreparedRun;
use super::runspec::{RunOutcome, StreamKind};
use super::spawn::spawn;
use anyhow::Result;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::time::Duration;

/// Once the child exits, the pipes may stay idle this long before the
/// supervisor kills the group, and again before it stops the readers.
const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(20);
/// Encoded frames waiting for a slow client. Past this, output readers stall.
const OUTGOING_CAP: usize = 256 * 1024;

#[cfg(target_os = "linux")]
const SEND_FLAGS: libc::c_int = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
#[cfg(not(target_os = "linux"))]
const SEND_FLAGS: libc::c_int = libc::MSG_DONTWAIT;

pub(crate) fn serve_run(mut stream: UnixStream, prepared: PreparedRun) -> Result<()> {
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    let PreparedRun {
        plan,
        fd_values,
        mask_values,
        short_keys,
    } = prepared;
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
        outgoing: Vec::new(),
        maskers: [
            Masker::new(mask_values.iter().cloned()),
            Masker::new(mask_values.iter().cloned()),
        ],
        open: [true, true],
        exited: None,
        exit_queued: false,
        credit: 0,
        idle: Duration::ZERO,
        group_killed: false,
    };
    drop(mask_values);
    for key in short_keys {
        let message = format!(
            "hush: {key} is shorter than 8 bytes, so it is not masked in this run's output"
        );
        supervisor.queue(ResponseFrame::RunNotice { message });
    }
    supervisor.queue(ResponseFrame::RunStdinCredit {
        bytes: STDIN_WINDOW as u32,
    });
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
        stream.try_clone()?,
        child.stdin,
        control_tx,
        Cancel::clone(&cancel),
    ));
    supervisor.supervise(&mut stream, &output, &control);
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    kill_group(child.pid, libc::SIGKILL);
    // Shutdown ends the client reader; dropping the receivers frees any blocked send.
    let _ = stream.shutdown(std::net::Shutdown::Both);
    drop((output, control, supervisor));
    for worker in workers {
        let _ = worker.join();
    }
    let mut status = 0;
    // SAFETY: the child is ours and the waiter only observed it with WNOWAIT.
    unsafe { libc::waitpid(child.pid, &mut status, 0) };
    Ok(())
}

struct Supervisor {
    pid: libc::pid_t,
    cancel: Cancel,
    outgoing: Vec<u8>,
    maskers: [Masker; 2],
    open: [bool; 2],
    exited: Option<RunOutcome>,
    exit_queued: bool,
    credit: usize,
    /// Time the pipes sat idle since the child exited, counted only while
    /// the supervisor was ready to take output.
    idle: Duration,
    group_killed: bool,
}

impl Supervisor {
    fn supervise(
        &mut self,
        stream: &mut UnixStream,
        output: &Receiver<Output>,
        control: &Receiver<Control>,
    ) {
        loop {
            while let Ok(event) = control.try_recv() {
                if !self.handle(event) {
                    return;
                }
            }
            if self.credit > 0 && self.outgoing.len() < OUTGOING_CAP {
                let bytes = self.credit as u32;
                self.queue(ResponseFrame::RunStdinCredit { bytes });
                self.credit = 0;
            }
            if let Some(outcome) = self.exited {
                if self.open == [false, false] {
                    if !self.exit_queued {
                        self.queue(ResponseFrame::RunExit { outcome });
                        self.exit_queued = true;
                    }
                } else if self.idle >= DRAIN_AFTER_EXIT {
                    // A descendant holds a pipe. Kill the group first; if a
                    // process outside it still holds one, stop the readers.
                    if self.group_killed {
                        self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                    } else {
                        kill_group(self.pid, libc::SIGKILL);
                        self.group_killed = true;
                    }
                    self.idle = Duration::ZERO;
                }
            }
            if !self.flush(stream) {
                return;
            }
            if self.exit_queued && self.outgoing.is_empty() {
                return;
            }
            if self.open != [false, false] && self.outgoing.len() < OUTGOING_CAP {
                match output.recv_timeout(TICK) {
                    Ok(Output::Chunk(kind, chunk)) => {
                        self.idle = Duration::ZERO;
                        let masked = self.maskers[index(kind)].push(&chunk);
                        self.queue_output(kind, masked);
                    }
                    Ok(Output::Eof(kind)) => self.close_stream(kind),
                    Err(RecvTimeoutError::Timeout) if self.exited.is_some() => self.idle += TICK,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => self.open = [false, false],
                }
            } else if !self.outgoing.is_empty() {
                wait_writable(stream);
            } else {
                match control.recv_timeout(TICK) {
                    Ok(event) => {
                        if !self.handle(event) {
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => std::thread::sleep(TICK),
                }
            }
        }
    }

    /// False when the client is gone.
    fn handle(&mut self, event: Control) -> bool {
        match event {
            Control::Exited(outcome) => self.exited = Some(outcome),
            Control::Signal(signal) => kill_group(self.pid, signal.number()),
            Control::StdinConsumed(bytes) => self.credit += bytes,
            Control::ClientGone => return false,
        }
        true
    }

    fn close_stream(&mut self, kind: StreamKind) {
        if self.open[index(kind)] {
            self.open[index(kind)] = false;
            let tail = self.maskers[index(kind)].finish();
            self.queue_output(kind, tail);
        }
    }

    fn queue_output(&mut self, kind: StreamKind, data: Vec<u8>) {
        if !data.is_empty() {
            self.queue(ResponseFrame::RunOutput {
                stream: kind,
                data: WireBytes(data),
            });
        }
    }

    fn queue(&mut self, frame: ResponseFrame) {
        // Encoding into a Vec fails only past the payload limit, which a
        // 32 KiB chunk, even fully masked, cannot reach.
        let _ = write_frame(&mut self.outgoing, &Frame::Response(frame));
    }

    /// Send what the socket takes right now, without blocking. False means the client is gone.
    fn flush(&mut self, stream: &UnixStream) -> bool {
        while !self.outgoing.is_empty() {
            // SAFETY: the pointer and length describe `outgoing`, which outlives the call.
            let sent = unsafe {
                libc::send(
                    stream.as_raw_fd(),
                    self.outgoing.as_ptr().cast(),
                    self.outgoing.len(),
                    SEND_FLAGS,
                )
            };
            match sent {
                0 => return false,
                sent if sent > 0 => {
                    self.outgoing.drain(..sent as usize);
                }
                _ => match std::io::Error::last_os_error().kind() {
                    std::io::ErrorKind::WouldBlock => return true,
                    std::io::ErrorKind::Interrupted => {}
                    _ => return false,
                },
            }
        }
        true
    }
}

fn wait_writable(stream: &UnixStream) {
    let mut poll = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };
    // SAFETY: one valid pollfd for the duration of the call.
    unsafe { libc::poll(&mut poll, 1, TICK.as_millis() as i32) };
}

fn index(kind: StreamKind) -> usize {
    match kind {
        StreamKind::Stdout => 0,
        StreamKind::Stderr => 1,
    }
}

fn kill_group(pid: libc::pid_t, signal: i32) {
    // SAFETY: the child leads its own process group, and its pid stays
    // reserved until the final reap, so -pid targets only that group.
    unsafe { libc::kill(-pid, signal) };
}
