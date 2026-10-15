//! A request the client abandons must stop promptly and write nothing,
//! whatever the client or another worker does to the descriptor it passed.
//! Each test plays the client on a real socket against the production handler.

use super::backend::BwsBackend;
use super::cancel_harness::Pending;
use super::daemon_harness::FakeAuth;
use super::fdpass_tests::pipe;
use super::human::HumanConfig;
use super::human_tests::{key, vault};
use super::operation::{Operation, Refusal, ValueChange, ValueSource};
use super::protocol::ResponseFrame;
use super::run_harness::tempdir;
use super::FakeBackend;
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(2);

fn add(key: &str) -> Operation {
    Operation::Add {
        project: "Test".into(),
        key: key.into(),
        note: None,
        value: ValueSource::InputFd,
    }
}

fn config() -> Arc<HumanConfig> {
    Arc::new(HumanConfig::new(FakeAuth::panicking()))
}

fn assert_cancelled(reply: Option<ResponseFrame>) {
    let cancelled = ResponseFrame::Refused {
        refusal: Refusal::ApprovalCancelled,
    };
    assert_eq!(reply, Some(cancelled));
}

/// A pipe that holds the whole value and its end of file. A large value needs
/// a feeder thread, because a macOS pipe buffers less than it.
fn ready(value: &[u8]) -> File {
    let (reader, mut writer) = pipe();
    if value.len() <= 4096 {
        writer.write_all(value).unwrap();
        return reader;
    }
    let value = value.to_vec();
    std::thread::spawn(move || writer.write_all(&value));
    reader
}

fn status_flags(file: &File) -> i32 {
    // SAFETY: F_GETFL only reads the status flags of an open descriptor.
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) }
}

/// The sender's own copy of the passed pipe shares its file status flags.
/// Wait briefly for a daemon that sets O_NONBLOCK, then clear the flag.
fn clear_nonblock_after_daemon(file: &File) {
    let until = Instant::now() + Duration::from_millis(500);
    while status_flags(file) & libc::O_NONBLOCK == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let flags = status_flags(file) & !libc::O_NONBLOCK;
    // SAFETY: F_SETFL changes only the status flags of an open descriptor.
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags) },
        0
    );
}

#[test]
fn a_buffered_add_from_a_client_that_left_writes_nothing() {
    let vault = vault();
    let input = ready(b"FAKE-buffered-0010");
    let mut pending = Pending::new(add("FRESH"), &[input.as_fd()]);
    pending.hang_up();
    let pending = pending.serve(vault.clone(), config());
    assert_cancelled(pending.reply_within(BOUND));
    assert_eq!(vault.write_count(), 0, "a cancelled add reached the vault");
}

#[test]
fn a_sender_that_clears_the_input_flag_cannot_hold_a_worker() {
    let vault = vault();
    let (reader, mut writer) = pipe();
    let mut pending = Pending::new(add("FRESH"), &[reader.as_fd()]).serve(vault.clone(), config());
    clear_nonblock_after_daemon(&reader);
    writer.write_all(b"x").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    pending.hang_up();
    assert_cancelled(pending.reply_within(BOUND));
    assert_eq!(vault.write_count(), 0);
}

#[test]
fn one_worker_leaving_cannot_reblock_another_workers_input() {
    let vault = vault();
    let (reader, mut writer) = pipe();
    let mut first = Pending::new(add("FIRST"), &[reader.as_fd()]).serve(vault.clone(), config());
    let until = Instant::now() + Duration::from_millis(500);
    while status_flags(&reader) & libc::O_NONBLOCK == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut second = Pending::new(add("SECOND"), &[reader.as_fd()]).serve(vault.clone(), config());
    std::thread::sleep(Duration::from_millis(100));
    first.hang_up();
    assert_cancelled(first.reply_within(BOUND));
    writer.write_all(b"x").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    second.hang_up();
    assert_cancelled(second.reply_within(BOUND));
    assert_eq!(vault.write_count(), 0);
}

#[test]
fn a_client_that_leaves_while_another_write_holds_the_lock_writes_nothing() {
    let (vault, config) = (vault(), config());
    let held = config.mutation.lock().unwrap();
    let input = ready(b"FAKE-locked-0011");
    let mut pending =
        Pending::new(add("FRESH"), &[input.as_fd()]).serve(vault.clone(), config.clone());
    std::thread::sleep(Duration::from_millis(200));
    pending.hang_up();
    let reply = pending.reply_within(BOUND);
    drop(held);
    std::thread::sleep(Duration::from_millis(200));
    assert_cancelled(reply);
    assert_eq!(vault.write_count(), 0);
}

#[test]
fn a_client_that_leaves_during_the_duplicate_lookup_writes_nothing() {
    let slow = Arc::try_unwrap(vault()).unwrap_or_else(|_| unreachable!());
    let vault = Arc::new(slow.with_catalog_delay(Duration::from_millis(400)));
    let input = ready(b"FAKE-slow-lookup-0012");
    let mut pending = Pending::new(add("FRESH"), &[input.as_fd()]).serve(vault.clone(), config());
    std::thread::sleep(Duration::from_millis(150));
    pending.hang_up();
    assert_cancelled(pending.reply_within(BOUND));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(vault.write_count(), 0);
}

/// A provider read that hangs must not outlive the client that asked.
#[test]
fn a_client_that_leaves_during_a_bws_read_is_not_kept_waiting() {
    let dir = tempdir("cancel-bws-read");
    let script = dir.join("bws");
    std::fs::write(&script, "#!/bin/sh\nsleep 5\nprintf '[]'\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let backend = Arc::new(BwsBackend::new(script, secrecy::SecretString::from("t")));
    let input = ready(b"FAKE-bws-read-0013");
    let mut pending = Pending::new(add("FRESH"), &[input.as_fd()]).serve(backend, config());
    std::thread::sleep(Duration::from_millis(200));
    let left = Instant::now();
    pending.hang_up();
    assert_cancelled(pending.reply_within(BOUND));
    assert!(left.elapsed() < BOUND, "waited {:?}", left.elapsed());
}

/// Keys and notes the provider would refuse are refused before the daemon
/// binds descriptors, looks anything up, or asks for anything.
#[test]
fn provider_invalid_keys_and_notes_are_refused_before_any_other_step() {
    let vault = vault();
    let long_key = "k".repeat(501);
    let long_note = "n".repeat(7001);
    let cases = [
        add(&long_key),
        Operation::Add {
            project: "Test".into(),
            key: "FRESH".into(),
            note: Some(long_note.clone()),
            value: ValueSource::InputFd,
        },
        Operation::Add {
            project: "Test".into(),
            key: "FRESH".into(),
            note: Some(" \t ".into()),
            value: ValueSource::InputFd,
        },
        Operation::Edit {
            secret: key("TOKEN"),
            new_key: Some(long_key.clone()),
            note: None,
            value: ValueChange::Keep,
        },
    ];
    for op in cases {
        let devnull = File::open("/dev/null").unwrap();
        let fds = vec![devnull.as_fd(); op.roles().len()];
        let pending = Pending::new(op.clone(), &fds).serve(vault.clone(), config());
        let reply = pending.reply_within(BOUND);
        let refused = matches!(
            &reply,
            Some(ResponseFrame::Refused {
                refusal: Refusal::InvalidKey { .. } | Refusal::InvalidValue { .. }
            })
        );
        assert!(refused, "{op:?} gave {reply:?}");
    }
    assert_eq!(vault.write_count(), 0);
    assert_eq!(vault.fetch_count(), 0);
}

#[test]
fn a_value_over_the_provider_character_limit_is_refused_before_writing() {
    let vault: Arc<FakeBackend> = vault();
    let input = ready("v".repeat(25_001).as_bytes());
    let pending = Pending::new(add("FRESH"), &[input.as_fd()]).serve(vault.clone(), config());
    let reply = pending.reply_within(BOUND);
    let refused = matches!(
        &reply,
        Some(ResponseFrame::Refused {
            refusal: Refusal::InvalidValue { .. }
        })
    );
    assert!(refused, "{reply:?}");
    assert_eq!(vault.write_count(), 0);
}
