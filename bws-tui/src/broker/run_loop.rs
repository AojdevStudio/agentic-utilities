//! Daemon run steps 6 to 8 from ADR 0002: start the child, move bytes, mask
//! output, forward stdin and signals, and report the exit status.

use super::mask::Masker;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame, WireBytes};
use super::run_prepare::PreparedRun;
use super::runspec::{ForwardSignal, RunOutcome, StreamKind};
use super::spawn::spawn;
use anyhow::Result;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Sender};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// After the child exits, wait this long for a grandchild to release its pipes.
const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);

const CHUNK: usize = 32 * 1024;

enum Event {
    Output(StreamKind, Zeroizing<Vec<u8>>),
    OutputEof(StreamKind),
    Exited(RunOutcome),
    Stdin(Vec<u8>),
    StdinEof,
    Signal(ForwardSignal),
    ClientGone,
}

pub(crate) fn serve_run(mut stream: UnixStream, prepared: PreparedRun) -> Result<()> {
    // A run may sit silent for hours; only a closed socket ends it early.
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
    let child = child?;
    let mut maskers = [
        Masker::new(mask_values.iter().cloned()),
        Masker::new(mask_values.iter().cloned()),
    ];
    drop(mask_values);
    for key in short_keys {
        let message = format!(
            "hush: {key} is shorter than 8 bytes, so it is not masked in this run's output"
        );
        let _ = write_frame(
            &mut stream,
            &Frame::Response(ResponseFrame::RunNotice { message }),
        );
    }
    for (mut pipe, value) in child.delivery.into_iter().zip(fd_values) {
        std::thread::spawn(move || {
            let _ = pipe.write_all(&value);
        });
    }
    let (events, inbox) = channel();
    spawn_reader(child.stdout, StreamKind::Stdout, events.clone());
    spawn_reader(child.stderr, StreamKind::Stderr, events.clone());
    spawn_client_reader(stream.try_clone()?, events.clone());
    let pid = child.pid;
    let waiter = events;
    std::thread::spawn(move || {
        let _ = waiter.send(Event::Exited(wait_child(pid)));
    });
    let stdin_tx = Some(spawn_stdin_writer(child.stdin));
    drive(&mut stream, pid, &inbox, &mut maskers, stdin_tx);
    Ok(())
}

fn drive(
    stream: &mut UnixStream,
    pid: libc::pid_t,
    inbox: &std::sync::mpsc::Receiver<Event>,
    maskers: &mut [Masker; 2],
    mut stdin_tx: Option<Sender<Vec<u8>>>,
) {
    let mut client_alive = true;
    let mut open = [true, true];
    let mut exited: Option<(RunOutcome, Instant)> = None;
    loop {
        if let Some((outcome, deadline)) = exited {
            if !client_alive {
                return;
            }
            if open == [false, false] || Instant::now() >= deadline {
                for kind in [StreamKind::Stdout, StreamKind::Stderr] {
                    let tail = maskers[index(kind)].finish();
                    send_output(stream, kind, tail);
                }
                send(stream, ResponseFrame::RunExit { outcome });
                return;
            }
        }
        let event = match exited {
            Some((_, deadline)) => {
                match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(event) => event,
                    Err(_) => continue,
                }
            }
            None => match inbox.recv() {
                Ok(event) => event,
                Err(_) => return,
            },
        };
        match event {
            Event::Output(kind, chunk) => {
                let masked = maskers[index(kind)].push(&chunk);
                if client_alive && !send_output(stream, kind, masked) {
                    client_alive = false;
                    kill_group(pid, libc::SIGKILL);
                }
            }
            Event::OutputEof(kind) => {
                open[index(kind)] = false;
                let tail = maskers[index(kind)].finish();
                if client_alive && !send_output(stream, kind, tail) {
                    client_alive = false;
                    kill_group(pid, libc::SIGKILL);
                }
            }
            Event::Exited(outcome) => exited = Some((outcome, Instant::now() + DRAIN_AFTER_EXIT)),
            Event::Stdin(data) => {
                if let Some(tx) = &stdin_tx {
                    let _ = tx.send(data);
                }
            }
            Event::StdinEof => stdin_tx = None,
            Event::Signal(signal) => kill_group(pid, signal.number()),
            Event::ClientGone => {
                if client_alive {
                    client_alive = false;
                    stdin_tx = None;
                    kill_group(pid, libc::SIGKILL);
                }
            }
        }
    }
}

fn index(kind: StreamKind) -> usize {
    match kind {
        StreamKind::Stdout => 0,
        StreamKind::Stderr => 1,
    }
}

fn send(stream: &mut UnixStream, frame: ResponseFrame) -> bool {
    write_frame(stream, &Frame::Response(frame)).is_ok()
}

fn send_output(stream: &mut UnixStream, kind: StreamKind, data: Vec<u8>) -> bool {
    data.is_empty()
        || send(
            stream,
            ResponseFrame::RunOutput {
                stream: kind,
                data: WireBytes(data),
            },
        )
}

fn kill_group(pid: libc::pid_t, signal: i32) {
    // SAFETY: the child leads its own process group, so -pid targets only that group.
    unsafe { libc::kill(-pid, signal) };
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

fn spawn_reader(mut pipe: File, kind: StreamKind, events: Sender<Event>) {
    std::thread::spawn(move || {
        let mut buf = vec![0u8; CHUNK];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if events
                        .send(Event::Output(kind, Zeroizing::new(buf[..read].to_vec())))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        buf.zeroize();
        let _ = events.send(Event::OutputEof(kind));
    });
}

fn spawn_client_reader(mut stream: UnixStream, events: Sender<Event>) {
    std::thread::spawn(move || loop {
        let event = match read_frame(&mut stream) {
            Ok(Frame::Request(RequestFrame::RunStdin { data })) => Event::Stdin(data.0),
            Ok(Frame::Request(RequestFrame::RunStdinEof)) => Event::StdinEof,
            Ok(Frame::Request(RequestFrame::RunSignal { signal })) => Event::Signal(signal),
            Ok(_) => continue,
            Err(_) => {
                let _ = events.send(Event::ClientGone);
                return;
            }
        };
        if events.send(event).is_err() {
            return;
        }
    });
}

fn spawn_stdin_writer(mut pipe: File) -> Sender<Vec<u8>> {
    let (tx, rx) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        for data in rx {
            if pipe.write_all(&data).is_err() {
                return;
            }
        }
    });
    tx
}
