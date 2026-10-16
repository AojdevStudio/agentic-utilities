//! Clean installs, no-op reruns and token custody on real trees.

use super::apply_harness::{all_bytes, install, Token, SENTINEL};
use super::fake_host_harness::{FakeHost, CIPHERTEXT};
use super::fixture_harness::Fixture;
use super::host::Action;
use super::layout::Platform;
use super::observe::observe;
use super::plan::Plan;
use super::status::Status;
use std::fs;
use std::os::unix::fs::MetadataExt;

fn mode(fixture: &Fixture, rel: &str) -> u32 {
    fs::symlink_metadata(fixture.path(rel)).unwrap().mode() & 0o7777
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[test]
fn a_clean_linux_install_creates_everything_once_and_a_rerun_changes_nothing() {
    let fixture = Fixture::new(Platform::Linux);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    let mut token = Token::default();
    let done = install(&fixture, &mut host, &mut token).unwrap();
    assert_eq!(done.len(), 10, "{done:?}");
    assert_eq!((token.reads, host.encrypted), (1, 1));
    for (rel, expected) in [
        ("etc/hush", 0o700),
        ("var/log/hush", 0o700),
        ("etc/hush/token.cred", 0o600),
        ("etc/pam.d/hush", 0o644),
        ("etc/systemd/system/hush.service", 0o644),
        ("var/lib/systemd/credential.secret", 0o400),
    ] {
        assert_eq!(mode(&fixture, rel), expected, "{rel}");
    }
    assert!(fs::read(fixture.path("etc/hush/token.cred"))
        .unwrap()
        .starts_with(CIPHERTEXT));
    assert!(
        !contains(&all_bytes(&fixture), SENTINEL),
        "plaintext token reached the tree"
    );
    assert_eq!(
        host.actions,
        [
            Action::CreateLinuxAccount {
                gid: 65534,
                shell: "/usr/sbin/nologin".into()
            },
            Action::CreateHostKey,
            Action::DaemonReload,
            Action::EnableUnit,
            Action::StartUnit,
        ]
    );
    let status = Status(&observe(fixture.layout(), &mut host)).to_string();
    assert!(
        status.starts_with("installed: yes\nenabled: yes\nactive: yes"),
        "{status}"
    );
    assert!(status.contains("reachable: yes"), "{status}");
    let before = fixture.snapshot();
    let plan = Plan::from_observation(observe(fixture.layout(), &mut host)).unwrap();
    assert!(plan.steps().is_empty(), "{:?}", plan.steps());
    assert!(install(&fixture, &mut host, &mut token).unwrap().is_empty());
    assert_eq!(
        (token.reads, host.actions.len()),
        (1, 5),
        "a rerun read input or touched the manager"
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_clean_macos_install_imports_one_record_and_loads_the_job() {
    let fixture = Fixture::new(Platform::MacOs);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::MacOs);
    let mut token = Token::default();
    install(&fixture, &mut host, &mut token).unwrap();
    assert_eq!(token.reads, 1);
    let scratch = fixture.temp.path().join("private/var/db/hush");
    assert_eq!(
        host.actions,
        [
            Action::CreateMacAccount {
                uid: 498,
                nobody_gid: -2,
                scratch: scratch.clone()
            },
            Action::LaunchdBootstrap
        ]
    );
    assert_eq!(
        fs::read_to_string(fixture.path("private/var/db/hush/token")).unwrap(),
        SENTINEL
    );
    for (rel, expected) in [
        ("private/var/db/hush", 0o700),
        ("private/var/db/hush/token", 0o600),
        ("Library/LaunchDaemons/hush.plist", 0o600),
    ] {
        assert_eq!(mode(&fixture, rel), expected, "{rel}");
    }
    let status = Status(&observe(fixture.layout(), &mut host)).to_string();
    assert!(
        status.starts_with("installed: yes\nenabled: yes\nactive: yes"),
        "{status}"
    );
    assert!(install(&fixture, &mut host, &mut token).unwrap().is_empty());
    assert_eq!((token.reads, host.actions.len()), (1, 2));
}

#[test]
fn an_existing_checked_credential_is_kept_and_no_input_is_read() {
    let fixture = Fixture::new(Platform::Linux);
    fixture.mkdir("etc/hush", 0o700);
    fixture.put("etc/hush/token.cred", b"operator ciphertext", 0o600);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    let mut token = Token::default();
    install(&fixture, &mut host, &mut token).unwrap();
    assert_eq!((token.reads, host.encrypted), (0, 0));
    assert_eq!(
        fs::read(fixture.path("etc/hush/token.cred")).unwrap(),
        b"operator ciphertext"
    );
}

#[test]
fn encryption_failure_stores_nothing_and_never_falls_back_to_plaintext() {
    let fixture = Fixture::new(Platform::Linux);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    host.fail_encrypt = true;
    let failure = install(&fixture, &mut host, &mut Token::default()).unwrap_err();
    assert!(failure.step.contains("token"), "{}", failure.step);
    assert_eq!(fs::read_dir(fixture.path("etc/hush")).unwrap().count(), 0);
    assert!(!contains(&all_bytes(&fixture), SENTINEL));
    assert_eq!(
        host.actions,
        [
            Action::CreateLinuxAccount {
                gid: 65534,
                shell: "/usr/sbin/nologin".into()
            },
            Action::CreateHostKey
        ]
    );
}

#[test]
fn failed_input_happens_before_any_write() {
    let fixture = Fixture::new(Platform::Linux);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    let before = fixture.snapshot();
    let mut token = Token {
        fail: true,
        ..Token::default()
    };
    let failure = install(&fixture, &mut host, &mut token).unwrap_err();
    assert!(failure.done.is_empty() && host.actions.is_empty());
    assert_eq!(fixture.snapshot(), before);
    assert!(!format!("{:#} {:?}", failure.error, failure.error).contains(SENTINEL));
}
