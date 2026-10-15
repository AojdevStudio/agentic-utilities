//! Token loader custody. Real files, synthetic token values.

use super::token::{self, TOKEN_MAX_BYTES};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

const SENTINEL: &str = "synthetic-token-sentinel-7f3a";

fn token_at(tag: &str, bytes: &[u8], mode: u32) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-token-test-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("token");
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn refusal(path: &PathBuf) -> String {
    let error = token::load_checked(path, true).expect_err("token must be refused");
    format!("{error:#} {error:?}")
}

#[test]
fn runtime_credential_mode_matches_the_platform() {
    let path = token_at("runtime-0400", format!("{SENTINEL}\n").as_bytes(), 0o400);
    let loaded = token::load_checked(&path, true);
    if cfg!(target_os = "linux") {
        use secrecy::ExposeSecret;
        assert_eq!(loaded.unwrap().expose_secret(), SENTINEL);
    } else {
        assert!(loaded.is_err(), "macOS keeps the token at exactly 0600");
    }
    let path = token_at("stored-0600", SENTINEL.as_bytes(), 0o600);
    assert!(token::load_checked(&path, true).is_ok());
}

#[test]
fn unsafe_modes_and_special_bits_are_refused() {
    for mode in [0o640, 0o604, 0o700, 0o4600, 0o2600, 0o1600, 0o200] {
        let path = token_at(&format!("mode-{mode:o}"), SENTINEL.as_bytes(), mode);
        assert!(
            token::load_checked(&path, true).is_err(),
            "mode {mode:o} must be refused"
        );
    }
}

#[test]
fn malformed_tokens_are_refused_without_their_bytes() {
    let mut oversized = SENTINEL.as_bytes().to_vec();
    oversized.resize(TOKEN_MAX_BYTES + 1, b'a');
    let mut invalid = SENTINEL.as_bytes().to_vec();
    invalid.push(0xff);
    let mut nul = SENTINEL.as_bytes().to_vec();
    nul.extend_from_slice(b"\0rest");
    for (tag, bytes) in [("oversized", oversized), ("utf8", invalid), ("nul", nul)] {
        let message = refusal(&token_at(tag, &bytes, 0o600));
        assert!(!message.contains(SENTINEL), "{tag} leaked: {message}");
        assert!(!message.contains("[115, 121"), "{tag} leaked raw bytes");
    }
    let mut at_limit = vec![b'a'; TOKEN_MAX_BYTES];
    at_limit[..SENTINEL.len()].copy_from_slice(SENTINEL.as_bytes());
    assert!(token::load_checked(&token_at("limit", &at_limit, 0o600), true).is_ok());
}

#[test]
fn a_fifo_candidate_is_refused_without_blocking() {
    let dir = std::env::temp_dir().join(format!("hush-token-test-{}-fifo", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("token");
    let raw = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    // SAFETY: mkfifo reads one valid C path.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(token::load_checked(&path, true).is_err());
    });
    let refused = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening a FIFO token must not block");
    assert!(refused);
}
