//! Token reads are bounded, refuse NUL and invalid UTF-8 with fixed errors,
//! and never block on a special file. Real files owned by the test user.

use super::private_temp_harness::PrivateTemp;
use super::token::{load_checked, TOKEN_LIMIT};
use secrecy::ExposeSecret;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

const SENTINEL: &[u8] = b"SYNTHETIC-SENTINEL-not-a-token";

fn private_file(dir: &PrivateTemp, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join("token");
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    path
}

/// Every rendering of the error chain, so a test sees what any caller could log.
fn renderings(error: &anyhow::Error) -> String {
    let causes: Vec<String> = error
        .chain()
        .map(|cause| format!("{cause} {cause:?}"))
        .collect();
    format!("{error:#} {error:?} {}", causes.join(" "))
}

fn assert_private(error: &anyhow::Error) {
    let text = renderings(error);
    let decimal = format!("{:?}", &SENTINEL[..8]);
    let decimal = decimal.trim_matches(|c| c == '[' || c == ']');
    assert!(!text.contains("SYNTHETIC-SENTINEL"), "{text}");
    assert!(!text.contains(decimal), "{text}");
}

#[test]
fn a_token_at_the_limit_loads_and_one_byte_more_is_refused() {
    let dir = PrivateTemp::new("hush-token-read-test");
    let path = private_file(&dir, &vec![b'a'; TOKEN_LIMIT]);
    assert_eq!(
        load_checked(&path, true).unwrap().expose_secret().len(),
        TOKEN_LIMIT
    );
    let mut oversized = SENTINEL.repeat(TOKEN_LIMIT / SENTINEL.len() + 1);
    oversized.truncate(TOKEN_LIMIT + 1);
    let path = private_file(&dir, &oversized);
    assert_private(&load_checked(&path, true).unwrap_err());
}

#[test]
fn nul_and_invalid_utf8_are_refused_without_the_bytes() {
    let dir = PrivateTemp::new("hush-token-read-test");
    for bad in [
        [SENTINEL, b"\0rest"].concat(),
        [SENTINEL, b"\xff\xfe"].concat(),
    ] {
        let path = private_file(&dir, &bad);
        assert_private(&load_checked(&path, true).unwrap_err());
    }
    let path = private_file(&dir, b" \n\t ");
    assert!(load_checked(&path, true).is_err());
}

#[test]
fn a_fifo_at_the_token_path_is_refused_without_blocking() {
    let dir = PrivateTemp::new("hush-token-read-test");
    let path = dir.path().join("token");
    let raw = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: `raw` is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    let (sender, receiver) = mpsc::channel();
    let fifo = path.clone();
    std::thread::spawn(move || sender.send(load_checked(&fifo, true).is_err()).unwrap());
    let outcome = receiver.recv_timeout(Duration::from_secs(2));
    if outcome.is_err() {
        // Opening the writer end releases the blocked reader thread.
        let _ = fs::OpenOptions::new().write(true).open(&path);
    }
    assert_eq!(outcome, Ok(true), "load blocked on a FIFO or accepted it");
}
