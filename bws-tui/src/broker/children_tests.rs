//! Child ownership stays bounded. A full table refuses at once, a slot comes
//! back only through a reap, and a value-input child holds nothing of the
//! daemon's but its two descriptors. No test here can make a child stay in an
//! uninterruptible kernel call, so retention is shown only up to the next reap.

use super::children::{exited_cleanly, Children};
use super::fdpass_tests::{pipe, writers_closed};
use super::input_pump::{read_input, InputStop};
use super::request::{Interrupt, RequestControl};
use super::spawn_fd::FdCeiling;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::{Duration, Instant};

#[expect(
    clippy::zombie_processes,
    reason = "the child table under test owns and reaps this pid"
)]
fn sleeper() -> libc::pid_t {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    child.id() as libc::pid_t
}

#[test]
fn a_full_table_refuses_at_once_and_admits_after_a_reap() {
    let table = Children::new(2);
    let mut first = table.reserve().unwrap().started(sleeper());
    let _second = table.reserve().unwrap().started(sleeper());
    let asked = Instant::now();
    assert!(table.reserve().is_none(), "a third child was admitted");
    assert!(asked.elapsed() < Duration::from_millis(100));
    let status = first.stop(Instant::now() + Duration::from_secs(2));
    assert!(status.is_some_and(|status| !exited_cleanly(status)));
    assert!(table.reserve().is_some(), "a reaped child kept its slot");
}

#[test]
fn an_unused_reservation_frees_its_slot() {
    let table = Children::new(1);
    drop(table.reserve().unwrap());
    assert_eq!(table.busy(), 0);
}

/// Drop never waits. A killed child it could not reap yet stays counted
/// until an admission reaps it.
#[test]
fn dropping_a_live_child_never_waits_and_never_loses_its_slot() {
    let table = Children::new(1);
    let child = table.reserve().unwrap().started(sleeper());
    let dropped = Instant::now();
    drop(child);
    assert!(dropped.elapsed() < Duration::from_millis(50), "drop waited");
    assert!(table.busy() <= 1);
    let until = Instant::now() + Duration::from_secs(2);
    while table.reserve().is_none() {
        assert!(Instant::now() < until, "the killed child was never reaped");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The reader child closes every descriptor but its two, so a pipe writer the
/// daemon holds elsewhere is not kept open by it.
#[test]
fn a_value_input_child_holds_no_other_daemon_descriptor() {
    let (watched, unrelated) = pipe();
    let (input, _stalled) = pipe();
    let (socket, client) = UnixStream::pair().unwrap();
    let reader = std::thread::spawn(move || {
        let control = RequestControl::until(&socket, Duration::from_secs(10));
        let (table, ceiling) = (Children::new(1), FdCeiling::establish().unwrap());
        let read = read_input(OwnedFd::from(input), &control, &table, ceiling).map(|v| v.len());
        (read, table.busy())
    });
    std::thread::sleep(Duration::from_millis(300));
    drop(unrelated);
    let closed = writers_closed(&watched);
    drop(client);
    let (read, busy) = reader.join().unwrap();
    assert!(
        closed,
        "the value-input child kept an unrelated pipe writer open"
    );
    assert_eq!(read, Err(InputStop::Stopped(Interrupt::Disconnected)));
    assert_eq!(
        busy, 0,
        "the stopped child was not reaped within the cleanup budget"
    );
}

/// This test binary's own `pipe` and `pipe2` can pause one thread right after
/// the syscall, before any later flag change.
#[cfg(target_os = "linux")]
mod pipe_window {
    use std::cell::Cell;
    use std::sync::mpsc::{Receiver, Sender};

    thread_local! {
        static PAUSE: Cell<Option<(Sender<()>, Receiver<()>)>> = const { Cell::new(None) };
    }

    pub(super) fn pause_next_pipe(created: Sender<()>, release: Receiver<()>) {
        PAUSE.set(Some((created, release)));
    }

    unsafe fn paused(fds: *mut libc::c_int, flags: libc::c_int) -> libc::c_int {
        let made = libc::syscall(libc::SYS_pipe2, fds, flags) as libc::c_int;
        if let Some((created, release)) = PAUSE.try_with(Cell::take).ok().flatten() {
            created.send(()).unwrap();
            release.recv().unwrap();
        }
        made
    }

    #[no_mangle]
    unsafe extern "C" fn pipe(fds: *mut libc::c_int) -> libc::c_int {
        paused(fds, 0)
    }

    #[no_mangle]
    unsafe extern "C" fn pipe2(fds: *mut libc::c_int, flags: libc::c_int) -> libc::c_int {
        paused(fds, flags)
    }
}

/// An exec that starts while the daemon creates its private value pipe must
/// not get the writer, or the complete value never reaches end of file.
#[cfg(target_os = "linux")]
#[test]
fn an_exec_during_value_pipe_creation_cannot_hold_its_writer() {
    use super::run_harness::tempdir;
    use std::sync::mpsc::channel;
    let dir = tempdir("value-pipe-window");
    let path = dir.join("value");
    std::fs::write(&path, b"FAKE-pipe-0001").unwrap();
    let input = OwnedFd::from(File::open(&path).unwrap());
    let (socket, _client) = UnixStream::pair().unwrap();
    let (created_tx, created) = channel();
    let (release, release_rx) = channel();
    let reader = std::thread::spawn(move || {
        let control = RequestControl::until(&socket, Duration::from_millis(500));
        let (table, ceiling) = (Children::new(1), FdCeiling::establish().unwrap());
        pipe_window::pause_next_pipe(created_tx, release_rx);
        read_input(input, &control, &table, ceiling).map(|value| value.len())
    });
    created.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut exec = Command::new("sleep").arg("2").spawn().unwrap();
    release.send(()).unwrap();
    let read = reader.join().unwrap();
    exec.kill().unwrap();
    exec.wait().unwrap();
    assert_eq!(read, Ok(14), "a concurrent exec held the value pipe writer");
}
