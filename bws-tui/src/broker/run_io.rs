//! Worker threads for one run. Every queue here is bounded: output by a
//! small channel, stdin by the credit window the client must respect.
//!
//! A failed channel send means the supervisor already finished the run, so
//! the worker stops. A failed pipe write means the child closed that end;
//! the bytes are dropped and stdin credit still returns.

use super::protocol::{read_frame, Frame, RequestFrame};
use super::runspec::{ForwardSignal, RunOutcome, StreamKind};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use zeroize::{Zeroize, Zeroizing};

/// Stdin bytes the client may have in flight before the daemon grants more.
pub(crate) const STDIN_WINDOW: usize = 256 * 1024;

const CHUNK: usize = 32 * 1024;

pub(super) enum Output {
    Chunk(StreamKind, Zeroizing<Vec<u8>>),
    Eof(StreamKind),
}

pub(super) enum Control {
    Exited(RunOutcome),
    Signal(ForwardSignal),
    StdinConsumed(usize),
    ClientGone,
}

pub(super) fn spawn_reader(mut pipe: File, kind: StreamKind, output: SyncSender<Output>) {
    std::thread::spawn(move || {
        let mut buf = vec![0u8; CHUNK];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let chunk = Zeroizing::new(buf[..read].to_vec());
                    buf[..read].zeroize();
                    if output.send(Output::Chunk(kind, chunk)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = output.send(Output::Eof(kind));
    });
}

pub(super) fn spawn_waiter(pid: libc::pid_t, control: SyncSender<Control>) {
    std::thread::spawn(move || {
        let _ = control.send(Control::Exited(wait_child(pid)));
    });
}

pub(super) fn spawn_delivery(mut pipe: File, value: Zeroizing<Vec<u8>>) {
    std::thread::spawn(move || {
        let _ = pipe.write_all(&value);
    });
}

/// A client that sends stdin past its window is treated as gone.
pub(super) fn spawn_client_reader(
    mut stream: UnixStream,
    stdin: File,
    control: SyncSender<Control>,
) {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let mut stdin_tx = Some(spawn_stdin_writer(
        stdin,
        Arc::clone(&in_flight),
        control.clone(),
    ));
    std::thread::spawn(move || loop {
        let event = match read_frame(&mut stream) {
            Ok(Frame::Request(RequestFrame::RunStdin { data })) => {
                let len = data.0.len();
                if in_flight.fetch_add(len, Ordering::SeqCst) + len > STDIN_WINDOW {
                    Control::ClientGone
                } else {
                    if let Some(tx) = &stdin_tx {
                        let _ = tx.send(data.0);
                    }
                    continue;
                }
            }
            Ok(Frame::Request(RequestFrame::RunStdinEof)) => {
                stdin_tx = None;
                continue;
            }
            Ok(Frame::Request(RequestFrame::RunSignal { signal })) => Control::Signal(signal),
            Ok(_) | Err(_) => Control::ClientGone,
        };
        let gone = matches!(event, Control::ClientGone);
        if control.send(event).is_err() || gone {
            return;
        }
    });
}

fn spawn_stdin_writer(
    mut pipe: File,
    in_flight: Arc<AtomicUsize>,
    control: SyncSender<Control>,
) -> std::sync::mpsc::Sender<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut open = true;
        for data in rx {
            open = open && pipe.write_all(&data).is_ok();
            in_flight.fetch_sub(data.len(), Ordering::SeqCst);
            if control.send(Control::StdinConsumed(data.len())).is_err() {
                return;
            }
        }
    });
    tx
}

fn wait_child(pid: libc::pid_t) -> RunOutcome {
    let mut status = 0;
    loop {
        // SAFETY: pid is this run's child; status is a valid out pointer.
        let rc = unsafe { libc::waitpid(pid, &mut status, 0) };
        if rc == pid {
            break;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return RunOutcome::Exited(127);
        }
    }
    if libc::WIFSIGNALED(status) {
        RunOutcome::Signaled(libc::WTERMSIG(status))
    } else {
        RunOutcome::Exited(libc::WEXITSTATUS(status))
    }
}
