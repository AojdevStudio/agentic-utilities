//! What a `bws` child can hold, and what the daemon keeps after it. A
//! timed-out call leaves no reader behind, and on macOS no stray daemon
//! descriptor reaches the child.

use super::backend::{BwsBackend, VaultBackend};
use super::meta::WriteFailure;
use super::request::{RequestControl, WriteAttempt};
use super::run_harness::tempdir;
use secrecy::SecretString;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

fn script(dir: &Path, body: &str) -> BwsBackend {
    let path = dir.join("bws");
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    BwsBackend::new(path, SecretString::from("t"))
}

/// A grandchild keeps the output pipe open after the daemon gives up. Once the
/// call returns, the daemon must hold no read end, so the late write fails.
#[test]
fn a_timed_out_bws_call_leaves_no_reader_on_its_output() {
    let dir = tempdir("bws-no-reader");
    let marker = dir.join("late-write-landed");
    let body = format!(
        "( sleep 1; printf late && : > '{}' ) &\nsleep 30\n",
        marker.display()
    );
    let backend = script(&dir, &body);
    let timeout = Duration::from_millis(300);
    let failed = backend.run_bws_with_deadline(&["secret", "list"], 1024, timeout);
    assert!(failed.is_err(), "the call should time out");
    assert_eq!(
        backend.busy_children(),
        0,
        "the killed bws child was not reaped"
    );
    std::thread::sleep(Duration::from_millis(1800));
    assert!(!marker.exists(), "a reader still held the bws output pipe");
}

/// macOS receives passed descriptors without close-on-exec and marks them one
/// call later. The child must get only its stdio either way.
#[cfg(target_os = "macos")]
#[test]
fn a_bws_child_inherits_no_stray_descriptor() {
    use std::os::fd::AsRawFd;
    let dir = tempdir("bws-no-stray-fd");
    let (reader, _writer) = super::fdpass_tests::pipe();
    // SAFETY: dup returns a new descriptor without close-on-exec, owned below.
    let stray = unsafe { libc::dup(reader.as_raw_fd()) };
    assert!(stray > 2);
    let marker = dir.join("inherited");
    let body = format!(
        "if [ -e /dev/fd/{stray} ]; then : > '{}'; fi\nprintf '[]'\n",
        marker.display()
    );
    let backend = script(&dir, &body);
    let listed = backend.run_bws_with_deadline(&["project", "list"], 1024, Duration::from_secs(10));
    // SAFETY: closes the descriptor this test duplicated.
    unsafe { libc::close(stray) };
    assert_eq!(listed.unwrap().as_str(), "[]");
    assert!(!marker.exists(), "fd {stray} reached the bws child");
}

/// One cleanup budget plus scheduling slack.
const LEAVE_BOUND: Duration = Duration::from_millis(500);

/// Runs `call` for a client that leaves once `bws` has closed its output.
/// Returns the result and how long after the leave the call ended, or `None`
/// when the call ended first, so a `bws` that never closes its output fails
/// the caller instead of hanging it.
fn call_then_leave<T>(
    closed: &Path,
    call: impl FnOnce(&RequestControl<'_>) -> T,
) -> (T, Option<Duration>) {
    let (socket, client) = UnixStream::pair().unwrap();
    let (ended, call_ended) = mpsc::channel::<()>();
    let closed = closed.to_path_buf();
    let left = std::thread::spawn(move || loop {
        if closed.exists() {
            drop(client);
            return Some(Instant::now());
        }
        match call_ended.recv_timeout(Duration::from_millis(1)) {
            Err(RecvTimeoutError::Timeout) => {}
            _ => return None,
        }
    });
    let control = RequestControl::until(&socket, Duration::from_secs(5));
    let result = call(&control);
    drop(ended);
    let late = left.join().unwrap().map(|at| at.elapsed());
    (result, late)
}

/// After `bws` closes its output it is still the request's child. A client
/// that leaves before it exits is not kept waiting, and a delete that may have
/// run is uncertain.
#[test]
fn a_client_that_leaves_after_bws_closes_its_output_is_not_kept_waiting() {
    let dir = tempdir("bws-eof-leave");
    let closed = dir.join("closed-output");
    let body = format!("exec 1>&-\n: > '{}'\nexec sleep 2\n", closed.display());
    let backend = script(&dir, &body);

    let (read, late) = call_then_leave(&closed, |control| backend.projects(control));
    let late = late.expect("bws never closed its output");
    let error = read.expect_err("a cancelled read returned data");
    assert!(
        format!("{error:#}").contains("the client left"),
        "the read ended for another reason: {error:#}"
    );
    assert!(late < LEAVE_BOUND, "the read held the worker {late:?}");

    std::fs::remove_file(&closed).unwrap();
    let (delete, late) = call_then_leave(&closed, |control| {
        backend.delete("s1", &mut WriteAttempt::new(control))
    });
    let late = late.expect("bws never closed its output");
    assert_eq!(delete, Err(WriteFailure::Uncertain));
    assert!(late < LEAVE_BOUND, "the delete held the worker {late:?}");
    assert_eq!(backend.busy_children(), 0, "the stopped bws was not reaped");
}

/// A `bws` that fails, or never starts, before it closes its output ends the
/// call on its own. The test must then report that, not wait for a client
/// leave that never comes.
#[test]
fn a_bws_that_ends_before_closing_its_output_fails_the_leave_check() {
    let dir = tempdir("bws-eof-never");
    let closed = dir.join("closed-output");
    let backend = script(&dir, "exit 3\n");
    let not_executable = std::fs::Permissions::from_mode(0o644);
    for (case, mode) in [("failed", None), ("never started", Some(not_executable))] {
        if let Some(mode) = mode {
            std::fs::set_permissions(dir.join("bws"), mode).unwrap();
        }
        let started = Instant::now();
        let (read, late) = call_then_leave(&closed, |control| backend.projects(control));
        assert!(read.is_err(), "a {case} bws returned data");
        assert_eq!(late, None, "a {case} bws reported a client leave");
        let waited = started.elapsed();
        assert!(
            waited < LEAVE_BOUND,
            "a {case} bws held the test {waited:?}"
        );
    }
    assert_eq!(backend.busy_children(), 0, "a failed bws was not reaped");
}
