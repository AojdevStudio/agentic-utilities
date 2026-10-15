//! Install behavior on real private trees with a fake native boundary.

use super::apply::{install_with, Halt};
use super::fake_host::{FakeHost, Input, TOKEN};
use super::layout::{definition_path, private_specs, token_source};
use super::model::{Backend, Fact, Manager};
use std::os::unix::fs::PermissionsExt;

fn backend() -> Backend {
    if cfg!(target_os = "macos") {
        Backend::File
    } else {
        Backend::Encrypted
    }
}

fn run(host: &mut FakeHost, input: &mut Input) -> Result<Vec<String>, Halt> {
    let layout = host.anchor.layout.clone();
    let binaries = host.binaries();
    install_with(&layout, &binaries, host, input)
}

fn installed_cleanly(host: &FakeHost) -> Vec<(std::path::PathBuf, u32, u64)> {
    let token = host.anchor.path(token_source(backend()));
    let bytes = std::fs::read(&token).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(TOKEN) || backend() == Backend::File);
    assert_eq!(std::fs::metadata(&token).unwrap().permissions().mode() & 0o7777, 0o600);
    for spec in private_specs(backend()) {
        assert!(host.anchor.path(&spec.path).exists(), "{}", spec.path.display());
    }
    assert!(host.anchor.path(definition_path()).exists());
    assert!(host.account.is_some());
    assert!(host.manager.enabled && host.manager.pid.is_some());
    host.anchor.snapshot()
}

#[test]
fn a_fresh_install_completes_and_a_rerun_changes_nothing() {
    let mut host = FakeHost::new("apply-fresh");
    let mut input = Input { token: Some(TOKEN), reads: 0 };
    let done = run(&mut host, &mut input).unwrap();
    assert_eq!(input.reads, 1);
    assert!(done.iter().any(|step| step.starts_with("write") && step.contains("token")));
    let tree = installed_cleanly(&host);
    let operations = host.log.clone();

    let mut closed = Input { token: None, reads: 0 };
    assert_eq!(run(&mut host, &mut closed).unwrap(), Vec::<String>::new());
    assert_eq!(closed.reads, 0, "an installed host reads no token");
    assert_eq!(host.log, operations, "an installed host gets no manager operation");
    assert_eq!(host.anchor.snapshot(), tree);
}

#[test]
fn a_live_managed_socket_is_a_noop_and_a_foreign_one_is_refused() {
    let mut host = FakeHost::new("apply-socket");
    run(&mut host, &mut Input { token: Some(TOKEN), reads: 0 }).unwrap();
    let socket = host.anchor.path(super::layout::socket_path());
    std::fs::create_dir_all(socket.parent().unwrap()).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    host.manager.pid = Some(std::process::id());
    let operations = host.log.clone();
    assert!(run(&mut host, &mut Input { token: None, reads: 0 }).unwrap().is_empty());
    assert_eq!(host.log, operations);

    host.manager.pid = Some(1);
    match run(&mut host, &mut Input { token: None, reads: 0 }) {
        Err(Halt::Refused(refusals)) => assert_eq!(refusals[0].subject, "broker socket"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_known_late_conflict_refuses_before_input_or_writes() {
    let mut host = FakeHost::new("apply-late");
    host.manager_fact = Some(Fact::Refused("the hush unit is masked"));
    let before = host.anchor.snapshot();
    let mut input = Input { token: Some(TOKEN), reads: 0 };
    assert!(matches!(run(&mut host, &mut input), Err(Halt::Refused(_))));
    assert_eq!(input.reads, 0);
    assert!(host.log.is_empty());
    assert_eq!(host.anchor.snapshot(), before);
}

#[test]
fn an_unknown_fact_refuses_before_input_or_writes() {
    let mut host = FakeHost::new("apply-unknown");
    host.account_fact = Some(Fact::Unknown("needs root to read the shadow file"));
    let before = host.anchor.snapshot();
    let mut input = Input { token: Some(TOKEN), reads: 0 };
    match run(&mut host, &mut input) {
        Err(Halt::Refused(refusals)) => assert!(refusals.iter().any(|r| r.unknown)),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(input.reads, 0);
    assert_eq!(host.anchor.snapshot(), before);
}

#[test]
fn a_change_during_apply_stops_before_later_steps() {
    for later in [Fact::Refused("the hush unit is masked"), Fact::Unknown("systemctl did not answer")] {
        let mut host = FakeHost::new("apply-changed");
        host.then_manager = Some(("create the hushrun account", later));
        match run(&mut host, &mut Input { token: Some(TOKEN), reads: 0 }) {
            Err(Halt::Changed { completed, .. }) => {
                assert!(completed.last().unwrap().contains("hushrun"));
            }
            other => panic!("expected a stop, got {other:?}"),
        }
        assert_eq!(host.log, ["create the hushrun account"]);
        assert!(!host.anchor.path(token_source(backend())).exists());
        assert!(!host.anchor.path(definition_path()).exists());
    }
}

#[test]
fn a_failure_at_each_native_step_resumes_to_the_clean_install() {
    let mut clean = FakeHost::new("apply-clean");
    run(&mut clean, &mut Input { token: Some(TOKEN), reads: 0 }).unwrap();
    let want = clean.anchor.snapshot().len();
    let mut points: Vec<&'static str> = vec![
        "create the hushrun account",
        "register the hush service",
        "enable the hush service",
        "start the hush service",
    ];
    if backend() == Backend::Encrypted {
        points.extend(["create the systemd credential host key", "encrypt"]);
    }
    for point in points {
        let mut host = FakeHost::new("apply-fault");
        host.fail = Some(point);
        let halted = run(&mut host, &mut Input { token: Some(TOKEN), reads: 0 });
        assert!(matches!(halted, Err(Halt::Failed { .. })), "{point}: {halted:?}");
        assert!(!host.log.iter().any(|done| done == point), "{point} ran");
        host.fail = None;
        let token_written = host.anchor.path(token_source(backend())).exists();
        let mut input = Input { token: Some(TOKEN), reads: 0 };
        run(&mut host, &mut input).unwrap_or_else(|halt| panic!("{point}: {halt:?}"));
        assert_eq!(input.reads, usize::from(!token_written), "{point}");
        assert_eq!(installed_cleanly(&host).len(), want, "{point}");
    }
}

#[test]
fn an_existing_checked_credential_reads_no_input() {
    let mut host = FakeHost::new("apply-existing");
    host.anchor.dirs(private_specs(backend())[0].path.clone());
    host.anchor.chmod(&private_specs(backend())[0].path, 0o700);
    host.anchor.file(token_source(backend()), b"existing synthetic source", 0o600);
    if backend() == Backend::Encrypted {
        host.anchor.file(super::layout::host_key_path(), b"key", 0o400);
    }
    let mut closed = Input { token: None, reads: 0 };
    let done = run(&mut host, &mut closed).unwrap();
    assert_eq!(closed.reads, 0);
    assert!(!done.iter().any(|step| step.contains("token")));
    let kept = std::fs::read(host.anchor.path(token_source(backend()))).unwrap();
    assert_eq!(kept, b"existing synthetic source");
}

#[test]
fn foreign_artifacts_are_refused_and_unrelated_files_kept() {
    let mut host = FakeHost::new("apply-foreign");
    host.anchor.file(definition_path(), b"[Service]\nExecStart=/bin/true\n", 0o644);
    let before = host.anchor.snapshot();
    assert!(matches!(run(&mut host, &mut Input { token: Some(TOKEN), reads: 0 }), Err(Halt::Refused(_))));
    assert_eq!(host.anchor.snapshot(), before);

    let mut host = FakeHost::new("apply-unrelated");
    let policy = host.anchor.file("/etc/hush/hushd.toml", b"require_hushrun = []\n", 0o600);
    host.anchor.chmod("/etc/hush", 0o700);
    host.manager = Manager { loaded: false, enabled: false, pid: None, stale: false };
    run(&mut host, &mut Input { token: Some(TOKEN), reads: 0 }).unwrap();
    assert_eq!(std::fs::read(policy).unwrap(), b"require_hushrun = []\n");
}
