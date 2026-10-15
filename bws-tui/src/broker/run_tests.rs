//! `hush run` end to end. Each test names one PR 2 behavior.

use super::client_run::RunInputs;
use super::run_harness::*;
use super::run_prepare::RunConfig;
use super::runspec::{Delivery, EnvName, ForwardSignal, RunOutcome, RunSpec};
use super::Client;
use std::ffi::OsString;
use std::path::Path;

fn run(dir: &Path, spec: RunSpec) -> (anyhow::Result<RunOutcome>, Capture) {
    let mut client = daemon(dir, RunConfig::default());
    let mut sink = Capture::default();
    let outcome = client.run(spec, &mut sink);
    (outcome, sink)
}

#[test]
fn env_delivery_reaches_child_and_output_is_masked() {
    let dir = tempdir("env");
    let script = format!("printf '%s\\n' \"$TOKEN\"; [ \"$TOKEN\" = '{ENV_VALUE}' ] && echo match");
    let (outcome, sink) = run(
        &dir,
        sh(
            vec![binding("TOKEN", "TOKEN", Delivery::Env)],
            &script,
            &dir,
        ),
    );
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "<hidden by hush>\nmatch\n");
    let audit = std::fs::read_to_string(dir.join("audit.log")).unwrap();
    assert!(
        audit.contains("run uid=") && audit.contains("secrets=TOKEN=s1"),
        "{audit}"
    );
    assert!(!audit.contains(ENV_VALUE));
}

#[test]
fn fd_delivery_is_readable_from_the_named_fd_and_absent_from_env() {
    let dir = tempdir("fd");
    let script = format!(
        "v=$(cat /dev/fd/$DBPASS_FD); [ \"$v\" = '{FD_VALUE}' ] && echo read-ok; \
         if env | grep -q -- '{FD_VALUE}'; then echo leaked; else echo env-clean; fi; echo fd=$DBPASS_FD"
    );
    let bindings = vec![binding("DB_PASSWORD", "DBPASS", Delivery::Fd)];
    let (outcome, sink) = run(&dir, sh(bindings, &script, &dir));
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "read-ok\nenv-clean\nfd=3\n");
}

#[test]
fn secret_split_across_writes_is_still_masked() {
    let dir = tempdir("split");
    let script = "printf 'a FAKE-run-'; sleep 0.3; printf 'value-0001 b\\n'";
    let (outcome, sink) = run(
        &dir,
        sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], script, &dir),
    );
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "a <hidden by hush> b\n");
}

#[test]
fn only_allowlisted_env_reaches_the_child() {
    let dir = tempdir("allowlist");
    let mut spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "env", &dir);
    for (name, value) in [
        ("TZ", "UTC"),
        ("LC_ALL", "C"),
        ("FOO_NOT_ALLOWED", "x"),
        ("BWS_ACCESS_TOKEN", "FAKE-not-a-token"),
        ("LD_FAKE_TEST", "1"),
        ("DYLD_FAKE_TEST", "1"),
    ] {
        spec.env_passthrough.push((
            EnvName::try_from(name.to_string()).unwrap(),
            OsString::from(value),
        ));
    }
    let (outcome, sink) = run(&dir, spec);
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    let env = sink.text();
    assert!(env.contains("TZ=UTC") && env.contains("LC_ALL=C"), "{env}");
    assert!(env.contains("TOKEN=<hidden by hush>"), "{env}");
    for banned in ["FOO_NOT_ALLOWED", "BWS_", "LD_FAKE", "DYLD_"] {
        assert!(!env.contains(banned), "{banned} reached the child: {env}");
    }
}

#[test]
fn binding_to_a_forbidden_name_is_refused() {
    let dir = tempdir("forbidden");
    let (outcome, _) = run(
        &dir,
        sh(
            vec![binding("TOKEN", "BWS_TOKEN", Delivery::Env)],
            "true",
            &dir,
        ),
    );
    assert!(outcome.unwrap_err().to_string().contains("BWS_"));
}

#[test]
fn exit_code_and_signal_death_propagate() {
    let dir = tempdir("exit");
    let token = || vec![binding("TOKEN", "TOKEN", Delivery::Env)];
    let (outcome, _) = run(&dir, sh(token(), "exit 7", &dir));
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(7));
    let dir = tempdir("signal");
    let (outcome, _) = run(&dir, sh(token(), "kill -TERM $$", &dir));
    assert_eq!(outcome.unwrap(), RunOutcome::Signaled(libc::SIGTERM));
}

#[test]
fn missing_working_directory_is_refused() {
    let dir = tempdir("cwd");
    let gone = dir.join("does-not-exist");
    let (outcome, _) = run(
        &dir,
        sh(
            vec![binding("TOKEN", "TOKEN", Delivery::Env)],
            "true",
            &gone,
        ),
    );
    assert!(outcome
        .unwrap_err()
        .to_string()
        .contains("working directory"));
}

#[test]
fn short_secret_warns_once_and_is_not_masked() {
    let dir = tempdir("short");
    let bindings = vec![
        binding("TINY", "TINY", Delivery::Env),
        binding("TINY", "TINY_COPY", Delivery::Fd),
    ];
    let (outcome, sink) = run(&dir, sh(bindings, "printf '%s' \"$TINY\"", &dir));
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "tiny1");
    assert_eq!(sink.notices.len(), 1, "{:?}", sink.notices);
    assert!(sink.notices[0].contains("TINY") && !sink.notices[0].contains("tiny1"));
}

#[test]
fn stdin_and_signals_are_forwarded() {
    let dir = tempdir("io");
    let mut client = daemon(&dir, RunConfig::default());
    let mut sink = Capture::default();
    let inputs = RunInputs {
        stdin: Some(Box::new(std::io::Cursor::new(b"piped-input\n".to_vec()))),
        signals: None,
    };
    let spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "cat", &dir);
    assert_eq!(
        client.run_with(spec, &mut sink, inputs).unwrap(),
        RunOutcome::Exited(0)
    );
    assert_eq!(sink.text(), "piped-input\n");

    let dir = tempdir("sig");
    let ready = dir.join("ready");
    let script = format!(
        "trap 'echo got-term; exit 42' TERM; echo up > '{}'; while :; do sleep 0.1; done",
        ready.display()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        assert!(wait_until(|| ready.exists()));
        tx.send(ForwardSignal::Term).unwrap();
    });
    let mut client = daemon(&dir, RunConfig::default());
    let mut sink = Capture::default();
    let inputs = RunInputs {
        stdin: None,
        signals: Some(rx),
    };
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        &script,
        &dir,
    );
    assert_eq!(
        client.run_with(spec, &mut sink, inputs).unwrap(),
        RunOutcome::Exited(42)
    );
    assert!(sink.text().contains("got-term"));
}

#[test]
fn client_disconnect_kills_the_child() {
    let dir = tempdir("disconnect");
    let pid_file = dir.join("pid");
    let script = format!("echo $$ > '{}'; exec sleep 30", pid_file.display());
    let stream =
        std::os::unix::net::UnixStream::connect(daemon_socket(&dir, RunConfig::default())).unwrap();
    let handle = stream.try_clone().unwrap();
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        &script,
        &dir,
    );
    std::thread::spawn(move || {
        let mut client = Client::from_stream(stream).unwrap();
        let _ = client.run(spec, &mut Capture::default());
    });
    assert!(wait_until(
        || std::fs::read_to_string(&pid_file).is_ok_and(|s| s.ends_with('\n'))
    ));
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    handle.shutdown(std::net::Shutdown::Both).unwrap();
    // SAFETY: signal 0 only checks that the process exists.
    assert!(
        wait_until(|| unsafe { libc::kill(pid, 0) } != 0),
        "child {pid} survived"
    );
}

#[test]
fn signals_ignored_by_the_daemon_are_reset_for_the_child() {
    let dir = tempdir("ignored-signal");
    let ready = dir.join("ready");
    // A daemon started in the background of a shell inherits ignored signals.
    // SAFETY: SIGHUP is not used by the test harness; the old disposition is restored below.
    let previous = unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) };
    let script = format!(
        "trap 'exit 43' HUP; echo up > '{}'; i=0; while [ $i -lt 50 ]; do sleep 0.1; i=$((i+1)); done; exit 9",
        ready.display()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        assert!(wait_until(|| ready.exists()));
        tx.send(ForwardSignal::Hup).unwrap();
    });
    let mut client = daemon(&dir, RunConfig::default());
    let inputs = RunInputs {
        stdin: None,
        signals: Some(rx),
    };
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        &script,
        &dir,
    );
    let outcome = client.run_with(spec, &mut Capture::default(), inputs);
    // SAFETY: restores the disposition saved above.
    unsafe { libc::signal(libc::SIGHUP, previous) };
    assert_eq!(outcome.unwrap(), RunOutcome::Exited(43));
}
