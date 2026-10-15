//! Client stdin failures: a read error fails the run and stops the child,
//! and an interrupted read is retried rather than treated as end of input.

use super::client_run::RunInputs;
use super::run_harness::*;
use super::run_prepare::RunConfig;
use super::runspec::{Delivery, RunOutcome};
use std::io::{Error, ErrorKind, Read};
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

fn run_with_stdin(
    tag: &str,
    script: &str,
    stdin: Scripted,
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
    let script = format!("echo $$ > '{}'; cat; exec sleep 30", pid_file.display());
    let stdin = Scripted(vec![
        Ok(b"partial"),
        Err(Error::from_raw_os_error(libc::EIO)),
    ]);
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
