//! Run supervision limits: bounded stdin, descendants that outlive the child,
//! start failures, and program lookup.

use super::client_run::RunInputs;
use super::run_harness::*;
use super::run_io::STDIN_WINDOW;
use super::run_prepare::{resolve_program, RunConfig};
use super::runspec::{Delivery, ForwardSignal, RunOutcome};
use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct Flood(Arc<AtomicUsize>);

impl Read for Flood {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        buf.fill(b'x');
        self.0.fetch_add(buf.len(), Ordering::SeqCst);
        Ok(buf.len())
    }
}

#[test]
fn stdin_to_a_child_that_never_reads_stays_bounded_and_signals_still_arrive() {
    let dir = tempdir("flood");
    let ready = dir.join("ready");
    let script = format!(
        "trap 'exit 44' TERM; echo up > '{}'; while :; do sleep 0.1; done",
        ready.display()
    );
    let sent = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        assert!(wait_until(|| ready.exists()));
        std::thread::sleep(std::time::Duration::from_millis(500));
        tx.send(ForwardSignal::Term).unwrap();
    });
    let mut client = daemon(&dir, RunConfig::default());
    let inputs = RunInputs {
        stdin: Some(Box::new(Flood(Arc::clone(&sent)))),
        signals: Some(rx),
    };
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        &script,
        &dir,
    );
    let outcome = client
        .run_with(spec, &mut Capture::default(), inputs)
        .unwrap();
    assert_eq!(outcome, RunOutcome::Exited(44));
    // The daemon queues at most one window; the kernel pipe to the child holds the rest.
    let read = sent.load(Ordering::SeqCst);
    assert!(
        read <= STDIN_WINDOW + (1 << 20),
        "client read {read} bytes of stdin"
    );
}

#[test]
fn descendants_holding_the_pipes_are_killed_when_the_run_ends() {
    let dir = tempdir("descendant");
    let pid_file = dir.join("bg");
    let script = format!("sleep 30 & echo $! > '{}'; echo done", pid_file.display());
    let mut client = daemon(&dir, RunConfig::default());
    let mut sink = Capture::default();
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        &script,
        &dir,
    );
    assert_eq!(client.run(spec, &mut sink).unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "done\n");
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // SAFETY: signal 0 only checks that the process exists.
    assert!(
        wait_until(|| unsafe { libc::kill(pid, 0) } != 0),
        "background {pid} survived"
    );
}

#[test]
fn a_child_that_cannot_start_reports_the_daemon_error() {
    let dir = tempdir("nostart");
    let mut spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "true", &dir);
    spec.program = "/nonexistent-hush-test/prog".into();
    let mut client = daemon(&dir, RunConfig::default());
    let error = client.run(spec, &mut Capture::default()).unwrap_err();
    assert!(error.to_string().contains("failed to start"), "{error}");
}

#[test]
fn relative_and_empty_path_entries_resolve_against_the_request_cwd() {
    let dir = tempdir("path");
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    for tool in [dir.join("bin/tool"), dir.join("here")] {
        std::fs::write(&tool, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let found = resolve_program(OsStr::new("tool"), &dir, Some(b"bin:/usr/bin")).unwrap();
    assert_eq!(found, dir.join("bin/tool"));
    let found = resolve_program(OsStr::new("here"), &dir, Some(b"/usr/bin::/bin")).unwrap();
    assert_eq!(found, dir.join("here"));
}

#[test]
fn fd_close_bound_covers_descriptors_above_the_soft_limit() {
    use super::spawn::close_bound;
    assert_eq!(close_bound(64, Some(100)), 101);
    assert_eq!(close_bound(256, Some(9)), 256);
    assert_eq!(close_bound(u64::MAX, None), 1 << 20);
}
