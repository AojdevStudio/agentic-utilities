//! Approved human operations on a real terminal. Values must appear only on
//! the terminal, never in a reply frame, client output, or the audit log.

use super::daemon_harness::{human_daemon, FakeAuth, PASSWORD};
use super::meta::SecretMeta;
use super::operation::{DescriptorProblem, Operation, Outcome, Refusal, SecretSelector};
use super::operation::{ValueChange, ValueSource};
use super::pty_harness::{Fd, Session, Step};
use super::run_harness::tempdir;
use super::FakeBackend;
use base64::Engine;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) const VALUE: &str = "FAKE-human-value-0003";

pub(super) fn vault() -> Arc<FakeBackend> {
    let entry = |id: &str, key: &str, value: &str| {
        let meta = SecretMeta {
            id: id.into(),
            key: key.into(),
            project: Some("Test".into()),
            project_id: Some("p1".into()),
            note: String::new(),
        };
        (id.to_string(), (meta, value.to_string()))
    };
    Arc::new(FakeBackend::new(HashMap::from([
        entry("s1", "TOKEN", VALUE),
        entry("s2", "OTHER", "FAKE-other-value-0005"),
    ])))
}

pub(super) fn key(key: &str) -> SecretSelector {
    SecretSelector::Key {
        key: key.into(),
        project: None,
    }
}

fn tty(op: Operation) -> Step {
    Step {
        op,
        fds: vec![Fd::Tty],
    }
}

#[test]
fn reveal_writes_the_value_only_to_the_terminal() {
    let dir = tempdir("human-reveal");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let mut session = Session::start(
        &socket,
        vec![tty(Operation::Reveal {
            secret: key("TOKEN"),
        })],
    );
    session.expect("Password for", 1);
    session.type_keys(&format!("{PASSWORD}\r"));
    assert!(
        session.wait_for(VALUE, 1),
        "value never reached the terminal"
    );
    session.expect("press Enter", 1);
    session.type_keys("\r");
    let done = session.finish();
    daemon.join().unwrap();
    let reply = serde_json::to_string(&done.results).unwrap();
    assert_eq!(
        done.results[0].reply,
        Ok(Outcome::Revealed {
            key: "TOKEN".into()
        })
    );
    assert!(!reply.contains(VALUE), "reply frame leaked the value");
    assert!(!done.stdout.contains(VALUE) && !done.stderr.contains(VALUE));
    assert!(!done.screen.contains(PASSWORD), "password was echoed");
    let audit = std::fs::read_to_string(dir.join("audit.log")).unwrap();
    assert!(
        audit.contains("op=reveal") && audit.contains("key=TOKEN"),
        "{audit}"
    );
    assert!(!audit.contains(VALUE) && !audit.contains(PASSWORD));
    assert_eq!((auth.calls(), vault.fetch_count()), (1, 1));
}

#[test]
fn wrong_password_and_cancel_fetch_nothing() {
    let dir = tempdir("human-deny");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let steps = vec![
        tty(Operation::Reveal {
            secret: key("TOKEN"),
        }),
        tty(Operation::Copy {
            secret: key("TOKEN"),
        }),
    ];
    let mut session = Session::start(&socket, steps);
    session.expect("Password for", 1);
    session.type_keys("not the password\r");
    session.expect("Password for", 2);
    session.type_keys("\x03");
    let done = session.finish();
    daemon.join().unwrap();
    let replies: Vec<_> = done.results.into_iter().map(|r| r.reply).collect();
    assert_eq!(
        replies,
        [
            Err(Refusal::ApprovalDenied),
            Err(Refusal::ApprovalCancelled)
        ]
    );
    assert_eq!((vault.fetch_count(), vault.write_count()), (0, 0));
    assert!(!done.screen.contains(VALUE));
}

#[test]
fn disconnect_during_approval_fetches_nothing() {
    let dir = tempdir("human-disconnect");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let mut session = Session::start(
        &socket,
        vec![tty(Operation::Reveal {
            secret: key("TOKEN"),
        })],
    );
    session.expect("Password for", 1);
    session.kill();
    daemon.join().unwrap();
    session.finish();
    assert_eq!((auth.calls(), vault.fetch_count()), (0, 0));
}

#[test]
fn edit_delete_and_copy_each_need_a_fresh_approval() {
    let dir = tempdir("human-fresh");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let steps = vec![
        tty(Operation::Copy {
            secret: key("TOKEN"),
        }),
        tty(Operation::Edit {
            secret: key("TOKEN"),
            new_key: None,
            note: Some("rotated".into()),
            value: ValueChange::Keep,
        }),
        tty(Operation::Delete {
            secret: key("OTHER"),
        }),
    ];
    let mut session = Session::start(&socket, steps);
    for prompt in 1..=3 {
        session.expect("Password for", prompt);
        session.type_keys(&format!("{PASSWORD}\r"));
    }
    let done = session.finish();
    daemon.join().unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(VALUE);
    assert!(done.screen.contains(&format!("\x1b]52;c;{encoded}\x07")));
    let replies: Vec<_> = done.results.into_iter().map(|r| r.reply).collect();
    assert_eq!(
        replies[0],
        Ok(Outcome::CopySent {
            key: "TOKEN".into()
        })
    );
    let Ok(Outcome::Edited { meta }) = &replies[1] else {
        panic!("edit failed: {replies:?}");
    };
    assert_eq!(
        (meta.key.as_str(), meta.note.as_str()),
        ("TOKEN", "rotated")
    );
    assert_eq!(
        replies[2],
        Ok(Outcome::Deleted {
            id: "s2".into(),
            key: "OTHER".into()
        })
    );
    assert_eq!(auth.calls(), 3, "no approval was reused");
    assert_eq!(
        vault.stored("s1").unwrap(),
        VALUE.as_bytes(),
        "metadata edit kept the value"
    );
    assert_eq!(vault.stored("s2"), None);
}

#[test]
fn a_replacement_value_is_read_from_the_terminal_after_approval() {
    let dir = tempdir("human-replace");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let replace = Operation::Edit {
        secret: key("TOKEN"),
        new_key: None,
        note: None,
        value: ValueChange::Replace(ValueSource::TtyPrompt),
    };
    let mut session = Session::start(&socket, vec![tty(replace)]);
    session.expect("Password for", 1);
    session.type_keys(&format!("{PASSWORD}\r"));
    session.expect("New value for TOKEN", 1);
    session.type_keys("FAKE-new-value-0004\r");
    let done = session.finish();
    daemon.join().unwrap();
    assert!(matches!(done.results[0].reply, Ok(Outcome::Edited { .. })));
    assert_eq!(vault.stored("s1").unwrap(), b"FAKE-new-value-0004");
    assert!(
        !done.screen.contains("FAKE-new-value-0004"),
        "value was echoed"
    );
}

#[test]
fn bad_descriptors_are_refused_before_any_prompt_or_fetch() {
    let dir = tempdir("human-descriptors");
    let (vault, auth) = (vault(), FakeAuth::new());
    let (socket, daemon) = human_daemon(&dir, vault.clone(), auth.clone(), 1);
    let reveal = |fds: Vec<Fd>| Step {
        op: Operation::Reveal {
            secret: key("TOKEN"),
        },
        fds,
    };
    let steps = vec![
        reveal(vec![Fd::Pipe("x".into())]),
        reveal(vec![Fd::DevNull]),
        reveal(vec![Fd::ForeignPty]),
        reveal(vec![]),
        reveal(vec![Fd::Tty, Fd::PipeWriter]),
    ];
    let done = Session::start(&socket, steps).finish();
    daemon.join().unwrap();
    let problem = |problem| Err(Refusal::Descriptors { problem });
    let replies: Vec<_> = done.results.iter().map(|r| r.reply.clone()).collect();
    assert_eq!(
        replies,
        [
            problem(DescriptorProblem::NotATerminal),
            problem(DescriptorProblem::NotATerminal),
            problem(DescriptorProblem::NotControlling),
            problem(DescriptorProblem::WrongCount {
                expected: 1,
                received: 0
            }),
            problem(DescriptorProblem::WrongCount {
                expected: 1,
                received: 2
            }),
        ]
    );
    assert_eq!(
        done.results[4].writer_closed,
        Some(true),
        "extra descriptor stayed open"
    );
    assert!(!done.screen.contains("Password for"));
    assert_eq!((auth.calls(), vault.fetch_count()), (0, 0));
}
