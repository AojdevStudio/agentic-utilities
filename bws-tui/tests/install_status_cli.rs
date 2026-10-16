//! The real hush binary admits install and status and dispatches them before any daemon is needed.
//! Stdin is a regular file shared with the child, so its offset shows whether the child read input.

#[path = "../src/broker/private_temp_harness.rs"]
mod private_temp_harness;

use private_temp_harness::PrivateTemp;
use std::fs::File;
use std::io::{Seek, Write};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const SYNTHETIC: &str = "SYNTHETIC-install-input-not-a-token\n";

struct Run {
    output: Output,
    stdin_offset: u64,
}

impl Run {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }
}

/// Wait for `child` until `limit`. A child still running then is killed and reaped,
/// so a hung command never outlives the test as an orphan or a zombie.
fn finish(mut child: Child, limit: Duration) -> Result<Output, String> {
    let deadline = Instant::now() + limit;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            return Err(format!("killed after {limit:?} and reaped with {status}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(child.wait_with_output().unwrap())
}

/// Run the built hush binary with synthetic stdin and a debug socket path where no daemon listens.
fn hush(subcommand: &str) -> Run {
    let dir = PrivateTemp::new("hush-cli-test");
    let input_path = dir.path().join("stdin");
    File::create(&input_path)
        .unwrap()
        .write_all(SYNTHETIC.as_bytes())
        .unwrap();
    let mut input = File::open(&input_path).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_hush"))
        .arg(subcommand)
        .env("HUSH_SOCKET", dir.path().join("absent").join("hush.sock"))
        .stdin(Stdio::from(input.try_clone().unwrap()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = finish(child, Duration::from_secs(20))
        .unwrap_or_else(|error| panic!("hush {subcommand}: {error}"));
    let stdin_offset = input.stream_position().unwrap();
    Run {
        output,
        stdin_offset,
    }
}

#[test]
fn a_timed_out_child_is_killed_and_reaped() {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id() as libc::pid_t;
    assert!(finish(child, Duration::from_millis(100)).is_err());
    // SAFETY: signal 0 only checks whether the pid still exists, a zombie included.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    assert!(!alive, "pid {pid} still exists after the timeout");
}

#[test]
fn status_reports_separate_facts_without_a_daemon_or_input() {
    let run = hush("status");
    let (stdout, stderr) = (run.stdout(), run.stderr());
    assert!(
        run.output.status.success(),
        "{:?}\n{stderr}",
        run.output.status
    );
    assert!(!stderr.contains("needs the hush broker daemon"), "{stderr}");
    let lines: Vec<String> = stdout
        .lines()
        .map(|line| line.trim().to_ascii_lowercase())
        .collect();
    for fact in [
        "installed",
        "enabled",
        "active",
        "reachable",
        "credential",
        "authentication",
    ] {
        assert!(
            lines.iter().any(|line| line.starts_with(fact)),
            "no {fact} line in {stdout}"
        );
    }
    let authentication = lines.iter().find(|line| line.starts_with("authentication"));
    assert!(
        authentication.is_some_and(|line| line.contains("not checked")),
        "{stdout}"
    );
    assert_eq!(run.stdin_offset, 0, "status read stdin");
    assert!(!stdout.contains(SYNTHETIC.trim()) && !stderr.contains(SYNTHETIC.trim()));
}

#[test]
fn nonroot_install_refuses_for_root_before_reading_input() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    assert_ne!(
        euid, 0,
        "this test runs the real install command, never run it as root"
    );
    let run = hush("install");
    let stderr = run.stderr();
    assert!(!run.output.status.success(), "nonroot install succeeded");
    assert_ne!(
        run.output.status.code(),
        Some(2),
        "usage error, not a refusal: {stderr}"
    );
    assert!(!stderr.contains("unrecognized subcommand"), "{stderr}");
    assert!(!stderr.contains("needs the hush broker daemon"), "{stderr}");
    assert!(stderr.to_ascii_lowercase().contains("root"), "{stderr}");
    assert_eq!(
        run.stdin_offset, 0,
        "install read input before its root refusal"
    );
    assert!(!run.stdout().contains(SYNTHETIC.trim()) && !stderr.contains(SYNTHETIC.trim()));
}
