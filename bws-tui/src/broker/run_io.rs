//! Worker threads for one run. Every queue here is bounded: output by a
//! small channel, stdin by the credit window the client must respect.
//!
//! Pipe workers use non-blocking pipes and wake every tick, so the supervisor
//! can cancel and join them even when a descendant holds a pipe open. A failed
//! channel send means the run already finished, so the worker stops. A failed
//! pipe write means the child closed that end; stdin credit still returns.

use super::protocol::{read_frame, Frame, RequestFrame};
use super::runspec::{ForwardSignal, RunOutcome, StreamKind};
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread::JoinHandle;
use zeroize::{Zeroize, Zeroizing};

/// Stdin bytes the client may have in flight before the daemon grants more.
pub(crate) const STDIN_WINDOW: usize = 256 * 1024;

const CHUNK: usize = 32 * 1024;
const TICK_MS: i32 = 20;

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

pub(super) type Cancel = Arc<AtomicBool>;

pub(super) fn spawn_reader(
    mut pipe: File,
    kind: StreamKind,
    output: SyncSender<Output>,
    cancel: Cancel,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = vec![0u8; CHUNK];
        if set_nonblocking(&pipe) {
            while ready(&pipe, libc::POLLIN, &cancel) {
                match pipe.read(&mut buf) {
                    Ok(0) => break,
                    Ok(read) => {
                        let chunk = Zeroizing::new(buf[..read].to_vec());
                        buf[..read].zeroize();
                        if output.send(Output::Chunk(kind, chunk)).is_err() {
                            return;
                        }
                    }
                    Err(error) if retry(&error) => {}
                    Err(_) => break,
                }
            }
        }
        let _ = output.send(Output::Eof(kind));
    })
}

/// Observe the exit without reaping, so the pid and its group id stay
/// reserved until the supervisor kills the group and reaps.
pub(super) fn spawn_waiter(pid: libc::pid_t, control: SyncSender<Control>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = control.send(Control::Exited(observe_exit(pid)));
    })
}

pub(super) fn spawn_delivery(
    pipe: File,
    value: Zeroizing<Vec<u8>>,
    cancel: Cancel,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        write_cancellable(&pipe, &value, &cancel);
    })
}

/// A client that sends stdin past its window is treated as gone.
pub(super) fn spawn_client_reader(
    mut stream: UnixStream,
    stdin: File,
    control: SyncSender<Control>,
    cancel: Cancel,
) -> [JoinHandle<()>; 2] {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let writer = {
        let (in_flight, control) = (Arc::clone(&in_flight), control.clone());
        std::thread::spawn(move || {
            let mut open = set_nonblocking(&stdin);
            for data in rx {
                open = open && write_cancellable(&stdin, &data, &cancel);
                in_flight.fetch_sub(data.len(), Ordering::SeqCst);
                if cancel.load(Ordering::SeqCst)
                    || control.send(Control::StdinConsumed(data.len())).is_err()
                {
                    return;
                }
            }
        })
    };
    let mut stdin_tx = Some(tx);
    let reader = std::thread::spawn(move || loop {
        let event = match read_frame(&mut stream) {
            Ok(Frame::Request(RequestFrame::RunStdin { data })) => {
                let len = data.0.len();
                if len == 0 {
                    continue;
                }
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
    [reader, writer]
}

fn write_cancellable(mut pipe: &File, mut data: &[u8], cancel: &AtomicBool) -> bool {
    while !data.is_empty() {
        if !ready(pipe, libc::POLLOUT, cancel) {
            return false;
        }
        match pipe.write(data) {
            Ok(written) => data = &data[written..],
            Err(error) if retry(&error) => {}
            Err(_) => return false,
        }
    }
    true
}

fn ready(file: &File, events: libc::c_short, cancel: &AtomicBool) -> bool {
    loop {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        if unsafe { libc::poll(&mut poll, 1, TICK_MS) } != 0 {
            return true;
        }
    }
}

fn retry(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted)
}

fn set_nonblocking(file: &File) -> bool {
    let fd = file.as_raw_fd();
    // SAFETY: fcntl reads and sets flags on an fd this worker owns.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0
    }
}

fn observe_exit(pid: libc::pid_t) -> RunOutcome {
    loop {
        // SAFETY: zeroed siginfo_t is a valid out buffer for waitid.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: pid is this run's child; WNOWAIT leaves it unreaped.
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if rc == 0 {
            // SAFETY: waitid filled a SIGCHLD siginfo, so si_status is valid.
            let status = unsafe { info.si_status() };
            return if info.si_code == libc::CLD_EXITED {
                RunOutcome::Exited(status)
            } else {
                RunOutcome::Signaled(status)
            };
        }
        if std::io::Error::last_os_error().kind() != ErrorKind::Interrupted {
            return RunOutcome::Exited(127);
        }
    }
}
