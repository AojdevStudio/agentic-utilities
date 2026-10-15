//! Child ownership stays bounded. A full table refuses at once, a slot comes
//! back only through a reap, and a value-input child holds nothing of the
//! daemon's but its two descriptors. No test here can make a child stay in an
//! uninterruptible kernel call, so retention is shown only up to the next reap.

use super::children::{exited_cleanly, Children};
use super::fdpass_tests::writers_closed;
use super::input_pump::{read_input, InputStop};
use super::request::{Interrupt, RequestControl};
use super::spawn_fd::FdCeiling;
use std::fs::File;
use std::os::fd::{FromRawFd, OwnedFd};
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

/// A pipe whose ends close on exec, so only a forked, not exec'd, child
/// could ever hold a copy.
fn cloexec_pipe() -> (File, File) {
    let mut fds = [0i32; 2];
    // SAFETY: pipe writes two new descriptors into a valid array.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    for fd in fds {
        // SAFETY: marks a descriptor owned here close-on-exec.
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }
    // SAFETY: both descriptors are new and owned only here.
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
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
    let (watched, unrelated) = cloexec_pipe();
    let (input, _stalled) = cloexec_pipe();
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
