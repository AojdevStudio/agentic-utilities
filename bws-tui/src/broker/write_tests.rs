//! Daemon write boundaries. The provider trims keys, so a key it would change
//! is refused before any lookup or write. A write the vault may have kept is
//! reported as uncertain, never as a plain failure.

use super::backend::{BwsBackend, VaultBackend};
use super::client::Client;
use super::daemon_harness::{human_daemon, FakeAuth, PASSWORD};
use super::fdpass_tests::pipe;
use super::human_tests::{key, vault, VALUE};
use super::meta::WriteFailure;
use super::operation::{Operation, Outcome, Refusal, ValueChange, ValueSource};
use super::pty_harness::{Fd, Session, Step};
use super::run_harness::tempdir;
use secrecy::SecretString;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

/// Every key `str::trim` changes. The SDK trims with the same rule.
const UNTRIMMED: [&str; 5] = ["TOKEN ", " TOKEN", "\u{a0}TOKEN", "TOKEN\u{3000}", " \t "];

fn add_bytes(socket: &Path, key: &str, value: &[u8]) -> Result<Outcome, Refusal> {
    let (reader, mut writer) = pipe();
    writer.write_all(value).unwrap();
    drop(writer);
    let op = Operation::Add {
        project: "Test".into(),
        key: key.into(),
        note: None,
        value: ValueSource::InputFd,
    };
    let mut client = Client::connect_to(&socket.to_path_buf()).unwrap();
    client.operate(op, &[reader.as_fd()]).unwrap()
}

fn approved_tty(socket: &Path, ops: Vec<Operation>) -> Vec<Result<Outcome, Refusal>> {
    let count = ops.len();
    let steps = ops
        .into_iter()
        .map(|op| Step {
            op,
            fds: vec![Fd::Tty],
        })
        .collect();
    let mut session = Session::start(socket, steps);
    for round in 1..=count {
        session.expect("Password for", round);
        session.type_keys(&format!("{PASSWORD}\r"));
    }
    let done = session.finish();
    done.results
        .into_iter()
        .map(|result| result.reply)
        .collect()
}

#[test]
fn add_refuses_keys_the_provider_would_trim() {
    let dir = tempdir("write-add-key");
    let vault = vault();
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::panicking(), 6);
    for untrimmed in UNTRIMMED {
        let reply = add_bytes(&socket, untrimmed, b"FAKE-trim-0010");
        assert_eq!(
            reply,
            Err(Refusal::InvalidKey {
                key: untrimmed.into()
            })
        );
    }
    let binary = add_bytes(&socket, "BINARY", b"FAKE-\xff-0011");
    assert!(
        matches!(binary, Err(Refusal::InvalidValue { .. })),
        "{binary:?}"
    );
    daemon.join().unwrap();
    assert_eq!(vault.write_count(), 0);
    assert_eq!(vault.stored("s1").unwrap(), VALUE.as_bytes());
}

#[test]
fn rename_refuses_keys_the_provider_would_trim() {
    let dir = tempdir("write-rename-key");
    let vault = vault();
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::new(), 1);
    let rename = |new_key: &str| Operation::Edit {
        secret: key("TOKEN"),
        new_key: Some(new_key.into()),
        note: None,
        value: ValueChange::Keep,
    };
    let replies = approved_tty(&socket, vec![rename("OTHER "), rename("\u{a0}OTHER")]);
    daemon.join().unwrap();
    assert_eq!(
        replies,
        [
            Err(Refusal::InvalidKey {
                key: "OTHER ".into()
            }),
            Err(Refusal::InvalidKey {
                key: "\u{a0}OTHER".into()
            }),
        ]
    );
    assert_eq!(vault.write_count(), 0);
    let keys: Vec<String> = vault
        .catalog()
        .unwrap()
        .into_iter()
        .map(|meta| meta.key)
        .collect();
    assert_eq!(keys, ["OTHER", "TOKEN"]);
}

#[test]
fn a_delete_whose_reply_is_lost_is_reported_uncertain() {
    let dir = tempdir("write-delete-lost");
    let vault = Arc::new(
        Arc::try_unwrap(vault())
            .unwrap_or_else(|_| unreachable!())
            .with_lost_delete_reply(),
    );
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::new(), 1);
    let delete = Operation::Delete {
        secret: key("TOKEN"),
    };
    let replies = approved_tty(&socket, vec![delete]);
    daemon.join().unwrap();
    assert_eq!(
        replies,
        [Err(Refusal::WriteUncertain {
            operation: "delete".into()
        })]
    );
    assert_eq!(vault.stored("s1"), None, "the fake committed the delete");
}

#[test]
fn a_bws_delete_that_started_and_failed_is_uncertain() {
    let dir = tempdir("write-bws-delete");
    let (script, marker) = (dir.join("bws"), dir.join("deleted"));
    let body = format!(
        "#!/bin/sh\nprintf '%s' \"$3\" > '{}'\nexit 1\n",
        marker.display()
    );
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let backend = BwsBackend::new(script, SecretString::from("t"));
    assert_eq!(backend.delete("s1"), Err(WriteFailure::Uncertain));
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap(),
        "s1",
        "bws ran the delete"
    );
    let missing = BwsBackend::new(dir.join("absent"), SecretString::from("t"));
    assert_eq!(
        missing.delete("s1"),
        Err(WriteFailure::Rejected),
        "never started"
    );
}
