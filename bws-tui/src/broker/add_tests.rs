//! Add needs no approval and never overwrites. Renames share the duplicate rule.

use super::client::Client;
use super::daemon_harness::{human_daemon, FakeAuth, PASSWORD};
use super::fdpass_tests::pipe;
use super::human_tests::{key, vault, VALUE};
use super::operation::{Operation, Outcome, Refusal, ValueChange, ValueSource};
use super::pty_harness::{Fd, Session, Step};
use super::run_harness::tempdir;
use super::VaultBackend;
use std::io::Write;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn add(key: &str) -> Operation {
    Operation::Add {
        project: "Test".into(),
        key: key.into(),
        note: None,
        value: ValueSource::InputFd,
    }
}

/// Add through a piped value, as `printf ... | hush add` does.
fn add_piped(socket: &Path, op: Operation, value: &str) -> Result<Outcome, Refusal> {
    let (reader, mut writer) = pipe();
    writer.write_all(value.as_bytes()).unwrap();
    drop(writer);
    let mut client = Client::connect_to(&socket.to_path_buf()).unwrap();
    client.operate(op, &[reader.as_fd()]).unwrap()
}

#[test]
fn add_needs_no_approval_and_refuses_an_existing_key() {
    let dir = tempdir("add-basic");
    let (vault, auth) = (vault(), FakeAuth::panicking());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth, 3);
    let added = add_piped(&socket, add("NEW_KEY"), "FAKE-added-value-0006\n");
    let Ok(Outcome::Added { meta }) = added else {
        panic!("add failed: {added:?}");
    };
    assert_eq!(meta.key, "NEW_KEY");
    assert_eq!(
        vault.stored(&meta.id).unwrap(),
        b"FAKE-added-value-0006\n",
        "bytes kept exactly"
    );
    let duplicate = add_piped(&socket, add("TOKEN"), "FAKE-overwrite-0007");
    assert_eq!(
        duplicate,
        Err(Refusal::AlreadyExists {
            key: "TOKEN".into(),
            id: "s1".into()
        })
    );
    let empty = add_piped(&socket, add("EMPTY_KEY"), "");
    assert!(
        matches!(empty, Err(Refusal::InvalidValue { .. })),
        "{empty:?}"
    );
    daemon.join().unwrap();
    assert_eq!(vault.write_count(), 1);
    assert_eq!(
        vault.stored("s1").unwrap(),
        VALUE.as_bytes(),
        "existing value untouched"
    );
}

#[test]
fn concurrent_adds_of_one_key_create_one_secret() {
    let dir = tempdir("add-race");
    let vault = Arc::new(
        Arc::try_unwrap(vault())
            .unwrap_or_else(|_| unreachable!())
            .with_write_delay(Duration::from_millis(200)),
    );
    let (socket, daemon) = human_daemon(&dir, vault.clone(), FakeAuth::panicking(), 2);
    let racers: Vec<_> = (0..2)
        .map(|index| {
            let socket = socket.clone();
            std::thread::spawn(move || {
                add_piped(&socket, add("RACE"), &format!("FAKE-race-{index}"))
            })
        })
        .collect();
    let mut replies: Vec<_> = racers
        .into_iter()
        .map(|racer| racer.join().unwrap())
        .collect();
    daemon.join().unwrap();
    replies.sort_by_key(|reply| reply.is_err());
    assert!(
        matches!(replies[0], Ok(Outcome::Added { .. })),
        "{replies:?}"
    );
    assert!(
        matches!(replies[1], Err(Refusal::AlreadyExists { .. })),
        "{replies:?}"
    );
    let race = vault
        .catalog()
        .unwrap()
        .into_iter()
        .filter(|meta| meta.key == "RACE")
        .count();
    assert_eq!(race, 1);
}

#[test]
fn add_prompts_on_the_terminal_when_no_value_is_piped() {
    let dir = tempdir("add-prompt");
    let (vault, auth) = (vault(), FakeAuth::panicking());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth, 1);
    let op = Operation::Add {
        project: "p1".into(),
        key: "TYPED_KEY".into(),
        note: Some("typed".into()),
        value: ValueSource::TtyPrompt,
    };
    let mut session = Session::start(
        &socket,
        vec![Step {
            op,
            fds: vec![Fd::Tty],
        }],
    );
    session.expect("Value for TYPED_KEY", 1);
    session.type_keys("FAKE-typed-value-0008\r");
    let done = session.finish();
    daemon.join().unwrap();
    let Ok(Outcome::Added { meta }) = &done.results[0].reply else {
        panic!("add failed: {:?}", done.results[0].reply);
    };
    assert_eq!(vault.stored(&meta.id).unwrap(), b"FAKE-typed-value-0008");
    assert!(
        !done.screen.contains("FAKE-typed-value-0008"),
        "value was echoed"
    );
    assert!(!done.screen.contains("Password for"));
}

#[test]
fn renaming_onto_an_existing_key_is_refused_after_approval() {
    let dir = tempdir("add-rename");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth, 1);
    let rename = Operation::Edit {
        secret: key("TOKEN"),
        new_key: Some("OTHER".into()),
        note: None,
        value: ValueChange::Keep,
    };
    let mut session = Session::start(
        &socket,
        vec![Step {
            op: rename,
            fds: vec![Fd::Tty],
        }],
    );
    session.expect("Password for", 1);
    session.type_keys(&format!("{PASSWORD}\r"));
    let done = session.finish();
    daemon.join().unwrap();
    assert_eq!(
        done.results[0].reply,
        Err(Refusal::AlreadyExists {
            key: "OTHER".into(),
            id: "s2".into()
        })
    );
    assert_eq!(vault.write_count(), 0);
}
