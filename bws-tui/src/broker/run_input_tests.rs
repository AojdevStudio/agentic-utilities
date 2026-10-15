//! Client stdin failures: a read error fails the run and stops the child,
//! and an interrupted or would-block read is retried rather than treated as
//! end of input.

use super::client_run::RunInputs;
use super::run_harness::*;
use super::run_prepare::RunConfig;
use super::runspec::{Delivery, RunOutcome};
use std::io::{Error, ErrorKind, Read};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Replays `steps` in order, then reports end of input.
struct Scripted(Vec<std::io::Result<&'static [u8]>>);

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() {
            return Ok(0);
        }
        let bytes = self.0.remove(0)?;
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
}

/// Sends some input, then fails with EIO once the child has written `started`.
struct FailsOnceStarted {
    started: std::path::PathBuf,
    sent: bool,
}

impl Read for FailsOnceStarted {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.sent {
            self.sent = true;
            buf[..7].copy_from_slice(b"partial");
            return Ok(7);
        }
        assert!(wait_until(|| self.started.exists()), "child never started");
        Err(Error::from_raw_os_error(libc::EIO))
    }
}

fn run_with_stdin(
    tag: &str,
    script: &str,
    stdin: impl Read + Send + 'static,
) -> (anyhow::Result<RunOutcome>, Capture) {
    let dir = tempdir(tag);
    let mut client = daemon(&dir, RunConfig::default());
    let spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], script, &dir);
    let inputs = RunInputs {
        stdin: Some(Box::new(stdin)),
        signals: None,
    };
    let mut sink = Capture::default();
    let outcome = client.run_with(spec, &mut sink, inputs);
    (outcome, sink)
}

#[test]
fn a_stdin_read_error_fails_the_run_and_stops_the_child() {
    let dir = tempdir("stdin-eio");
    let pid_file = dir.join("pid");
    let partial = dir.join("pid.partial");
    let script = format!(
        "echo $$ > '{0}'; mv '{0}' '{1}'; cat; exec sleep 30",
        partial.display(),
        pid_file.display()
    );
    let stdin = FailsOnceStarted {
        started: pid_file.clone(),
        sent: false,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_with_stdin("stdin-eio-run", &script, stdin).0);
    });
    let outcome = rx.recv_timeout(Duration::from_secs(10));
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let error = match outcome {
        Ok(Err(error)) => error,
        other => {
            if pid > 0 {
                // SAFETY: SIGKILL to this test's own leftover child.
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
            panic!("a stdin read error must fail the run promptly, got {other:?}");
        }
    };
    assert!(
        format!("{error:#}").contains(&Error::from_raw_os_error(libc::EIO).to_string()),
        "{error:#}"
    );
    assert!(pid > 0, "the child never started");
    // SAFETY: signal 0 only checks that the process exists.
    assert!(
        wait_until(|| unsafe { libc::kill(pid, 0) } != 0),
        "child {pid} survived a failed run"
    );
}

#[test]
fn an_interrupted_stdin_read_is_retried() {
    let stdin = Scripted(vec![Err(ErrorKind::Interrupted.into()), Ok(b"hello\n")]);
    let (outcome, sink) = run_with_stdin("stdin-eintr", "cat", stdin);
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "hello\n");
}

#[test]
fn a_would_block_stdin_read_is_retried() {
    let stdin = Scripted(vec![Err(ErrorKind::WouldBlock.into()), Ok(b"hello\n")]);
    let (outcome, sink) = run_with_stdin("stdin-eagain", "cat", stdin);
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "hello\n");
}

/// A non-blocking stdin that never has data. Counts its reads.
struct NeverReady(Arc<AtomicUsize>);

impl Read for NeverReady {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ErrorKind::WouldBlock.into())
    }
}

#[test]
fn a_stdin_that_never_has_data_waits_without_spinning_and_stops_with_the_run() {
    let reads = Arc::new(AtomicUsize::new(0));
    let stdin = NeverReady(Arc::clone(&reads));
    let (outcome, sink) = run_with_stdin("stdin-never", "sleep 0.3; echo done", stdin);
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "done\n");
    let during = reads.load(Ordering::SeqCst);
    assert!(
        (1..=50).contains(&during),
        "{during} reads while the run lasted"
    );
    std::thread::sleep(Duration::from_millis(100));
    let settled = reads.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        reads.load(Ordering::SeqCst),
        settled,
        "stdin worker outlived the run"
    );
}
