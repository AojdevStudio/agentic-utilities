//! Daemon run steps 6 to 8 from ADR 0002: start the child, move bytes, mask
//! output, forward stdin and signals, and report the exit status.
//!
//! The supervisor never blocks on the client for longer than one tick, so a
//! forwarded signal or a disconnect is handled while output waits. When the
//! run ends, every process left in the child's group is killed.

use super::mask::Masker;
use super::protocol::{write_frame, Frame, ResponseFrame, WireBytes};
use super::run_io::{
    spawn_client_reader, spawn_delivery, spawn_reader, spawn_waiter, Control, Output, STDIN_WINDOW,
};
use super::run_prepare::PreparedRun;
use super::runspec::{RunOutcome, StreamKind};
use super::spawn::spawn;
use anyhow::Result;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// After the child exits, descendants get this long to release the output pipes.
const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(20);
/// Encoded frames waiting for a slow client. Past this, output readers stall.
const OUTGOING_CAP: usize = 256 * 1024;

pub(crate) fn serve_run(mut stream: UnixStream, prepared: PreparedRun) -> Result<()> {
    stream.set_read_timeout(None)?;
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
    let mut supervisor = Supervisor {
        pid: child.pid,
        outgoing: Vec::new(),
        maskers: [
            Masker::new(mask_values.iter().cloned()),
            Masker::new(mask_values.iter().cloned()),
        ],
        open: [true, true],
        exited: None,
        exit_queued: false,
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
    for (pipe, value) in child.delivery.into_iter().zip(fd_values) {
        spawn_delivery(pipe, value);
    }
    let (output_tx, output) = sync_channel(16);
    let (control_tx, control) = sync_channel(256);
    spawn_reader(child.stdout, StreamKind::Stdout, output_tx.clone());
    spawn_reader(child.stderr, StreamKind::Stderr, output_tx);
    spawn_waiter(child.pid, control_tx.clone());
    spawn_client_reader(stream.try_clone()?, child.stdin, control_tx);
    stream.set_write_timeout(Some(TICK))?;
    supervisor.supervise(&mut stream, &output, &control);
    kill_group(child.pid, libc::SIGKILL);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

struct Supervisor {
    pid: libc::pid_t,
    outgoing: Vec<u8>,
    maskers: [Masker; 2],
    open: [bool; 2],
    exited: Option<(RunOutcome, Instant)>,
    exit_queued: bool,
}

impl Supervisor {
    fn supervise(
        &mut self,
        stream: &mut UnixStream,
        output: &Receiver<Output>,
        control: &Receiver<Control>,
    ) {
        let mut credit = 0;
        let mut group_killed = false;
        loop {
            while let Ok(event) = control.try_recv() {
                match event {
                    Control::Exited(outcome) => {
                        self.exited = Some((outcome, Instant::now() + DRAIN_AFTER_EXIT));
                    }
                    Control::Signal(signal) => kill_group(self.pid, signal.number()),
                    Control::StdinConsumed(bytes) => credit += bytes,
                    Control::ClientGone => return,
                }
            }
            if credit > 0 {
                self.queue(ResponseFrame::RunStdinCredit {
                    bytes: credit as u32,
                });
                credit = 0;
            }
            if let Some((outcome, deadline)) = self.exited {
                if self.open == [false, false] {
                    if !self.exit_queued {
                        self.queue(ResponseFrame::RunExit { outcome });
                        self.exit_queued = true;
                    }
                } else if Instant::now() >= deadline && !group_killed {
                    kill_group(self.pid, libc::SIGKILL);
                    group_killed = true;
                } else if Instant::now() >= deadline + DRAIN_AFTER_EXIT {
                    // A descendant outside the group still holds a pipe. Stop waiting.
                    self.close_stream(StreamKind::Stdout);
                    self.close_stream(StreamKind::Stderr);
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
                        let masked = self.maskers[index(kind)].push(&chunk);
                        self.queue_output(kind, masked);
                    }
                    Ok(Output::Eof(kind)) => self.close_stream(kind),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => self.open = [false, false],
                }
            } else if self.outgoing.is_empty() {
                std::thread::sleep(TICK);
            }
        }
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

    /// Write what the socket takes within one tick. False means the client is gone.
    fn flush(&mut self, stream: &mut UnixStream) -> bool {
        while !self.outgoing.is_empty() {
            match stream.write(&self.outgoing) {
                Ok(0) => return false,
                Ok(written) => {
                    self.outgoing.drain(..written);
                }
                Err(error) => match error.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => return true,
                    std::io::ErrorKind::Interrupted => {}
                    _ => return false,
                },
            }
        }
        true
    }
}

fn index(kind: StreamKind) -> usize {
    match kind {
        StreamKind::Stdout => 0,
        StreamKind::Stderr => 1,
    }
}

fn kill_group(pid: libc::pid_t, signal: i32) {
    // SAFETY: the child leads its own process group, so -pid targets only that group.
    unsafe { libc::kill(-pid, signal) };
}
