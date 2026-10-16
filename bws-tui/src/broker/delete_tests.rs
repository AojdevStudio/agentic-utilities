//! A delete removes only the secret the human approved. The vault is read
//! again under the mutation lock just before the write, so a secret renamed,
//! moved, or removed while the password prompt waited is refused, and two
//! daemon writes never overlap.

use super::daemon_harness::{human_daemon, FakeAuth, PASSWORD};
use super::human::HumanConfig;
use super::human_tests::{key, vault, VALUE};
use super::meta::SecretMeta;
use super::operation::{Operation, Outcome, Refusal};
use super::pty_harness::{Fd, Session, Step};
use super::run_harness::tempdir;
use super::{audit, daemon, FakeBackend};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

fn delete_token() -> Vec<Step> {
    let op = Operation::Delete {
        secret: key("TOKEN"),
    };
    vec![Step {
        op,
        fds: vec![Fd::Tty],
    }]
}

/// Delete TOKEN while `meanwhile` changes the vault during the password prompt.
fn delete_after(
    tag: &str,
    meanwhile: impl FnOnce(&FakeBackend),
) -> (Arc<FakeBackend>, Result<Outcome, Refusal>) {
    let dir = tempdir(tag);
    let vault = vault();
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::new(), 1);
    let mut session = Session::start(&socket, delete_token());
    session.expect("Password for", 1);
    meanwhile(&vault);
    session.type_keys(&format!("{PASSWORD}\r"));
    let mut done = session.finish();
    daemon.join().unwrap();
    (vault, done.results.remove(0).reply)
}

fn change_token(vault: &FakeBackend, change: impl FnOnce(&mut SecretMeta)) {
    vault.outside_write(|secrets| change(&mut secrets.get_mut("s1").unwrap().0));
}

/// The approved secret is gone. Nothing was written and the value is kept.
fn assert_refused_and_kept(vault: &FakeBackend, reply: Result<Outcome, Refusal>) {
    let refused = Err(Refusal::NotFound {
        key: "TOKEN".into(),
    });
    assert_eq!(reply, refused);
    assert_eq!(vault.write_count(), 0);
    assert_eq!(vault.stored("s1").unwrap(), VALUE.as_bytes());
}

/// One connection served with a mutation lock the test can hold.
fn serve_one(
    dir: &Path,
    vault: Arc<FakeBackend>,
    human: Arc<HumanConfig>,
) -> (PathBuf, JoinHandle<()>) {
    let socket = dir.join("hush.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let audit = audit::FileAuditLog::open(&dir.join("audit.log")).unwrap();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let run = super::run_prepare::RunConfig::default();
        let _ = daemon::serve_connection_with(stream, &*vault, &audit, &run, &human);
    });
    (socket, handle)
}

#[test]
fn an_unchanged_secret_is_deleted() {
    let (vault, reply) = delete_after("delete-unchanged", |_| {});
    let deleted = Outcome::Deleted {
        id: "s1".into(),
        key: "TOKEN".into(),
    };
    assert_eq!(reply, Ok(deleted));
    assert_eq!(vault.stored("s1"), None);
}

#[test]
fn a_secret_renamed_during_the_prompt_is_not_deleted() {
    let (vault, reply) = delete_after("delete-renamed", |vault| {
        change_token(vault, |meta| meta.key = "RENAMED".into());
    });
    assert_refused_and_kept(&vault, reply);
}

#[test]
fn a_secret_moved_to_another_project_during_the_prompt_is_not_deleted() {
    let (vault, reply) = delete_after("delete-moved", |vault| {
        change_token(vault, |meta| {
            meta.project_id = Some("p2".into());
            meta.project = Some("Elsewhere".into());
        });
    });
    assert_refused_and_kept(&vault, reply);
}

/// The prompt showed the project name, so a renamed project is a different approval.
#[test]
fn a_secret_whose_project_was_renamed_during_the_prompt_is_not_deleted() {
    let (vault, reply) = delete_after("delete-project-renamed", |vault| {
        change_token(vault, |meta| meta.project = Some("Renamed".into()));
    });
    assert_refused_and_kept(&vault, reply);
}

#[test]
fn a_secret_removed_during_the_prompt_is_reported_missing() {
    let (vault, reply) = delete_after("delete-removed", |vault| {
        vault.outside_write(|secrets| {
            secrets.remove("s1");
        });
    });
    let missing = Err(Refusal::NotFound {
        key: "TOKEN".into(),
    });
    assert_eq!(reply, missing);
    assert_eq!(vault.write_count(), 0);
}

#[test]
fn a_delete_waits_for_another_daemon_write() {
    let dir = tempdir("delete-serial");
    let (vault, human) = (vault(), Arc::new(HumanConfig::new(FakeAuth::new())));
    let (socket, daemon) = serve_one(&dir, vault.clone(), human.clone());
    let held = human.mutation.lock().unwrap();
    let mut session = Session::start(&socket, delete_token());
    session.expect("Password for", 1);
    session.type_keys(&format!("{PASSWORD}\r"));
    std::thread::sleep(Duration::from_millis(300));
    let early = vault.write_count();
    drop(held);
    let mut done = session.finish();
    daemon.join().unwrap();
    assert_eq!(early, 0, "the delete ran while another write held the lock");
    let deleted = Outcome::Deleted {
        id: "s1".into(),
        key: "TOKEN".into(),
    };
    assert_eq!(done.results.remove(0).reply, Ok(deleted));
    assert_eq!(vault.stored("s1"), None);
}

#[test]
fn a_delete_whose_client_leaves_while_waiting_for_the_lock_writes_nothing() {
    let dir = tempdir("delete-leave");
    let (vault, human) = (vault(), Arc::new(HumanConfig::new(FakeAuth::new())));
    let (socket, daemon) = serve_one(&dir, vault.clone(), human.clone());
    let held = human.mutation.lock().unwrap();
    let mut session = Session::start(&socket, delete_token());
    session.expect("Password for", 1);
    session.type_keys(&format!("{PASSWORD}\r"));
    std::thread::sleep(Duration::from_millis(300));
    session.kill();
    let _ = session.finish();
    let (sender, stopped) = channel();
    std::thread::spawn(move || sender.send(daemon.join()));
    let stopped = stopped.recv_timeout(Duration::from_secs(2));
    drop(held);
    assert!(
        stopped.is_ok(),
        "the worker kept waiting after the client left"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(vault.write_count(), 0);
    assert_eq!(vault.stored("s1").unwrap(), VALUE.as_bytes());
}
