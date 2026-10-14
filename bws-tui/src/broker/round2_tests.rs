//! Regression tests for the broker hardening round 2.

use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-round2-test-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn bws_ancestor_directory_group_writable_is_refused_on_disk() {
    let dir = tempdir("bws-ancestor");
    let inner = dir.join("inner");
    fs::create_dir_all(&inner).unwrap();
    let deep = inner.join("bin");
    fs::create_dir_all(&deep).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o775)).unwrap();
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&deep, fs::Permissions::from_mode(0o755)).unwrap();
    let script = deep.join("bws");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(paths::validate_trusted_bws_path(&script, true).is_err());
}

#[test]
fn existing_socket_dir_world_writable_is_refused_unchanged() {
    let dir = tempdir("socket-existing");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
    let socket = dir.join("hush.sock");
    assert!(server::bind_listener(&socket).is_err());
    assert_eq!(fs::metadata(&dir).unwrap().mode() & 0o777, 0o777);
    assert!(!socket.exists());
}

#[test]
fn fresh_socket_dir_gets_0755() {
    let dir = tempdir("socket-fresh");
    let sock_dir = dir.join("run");
    let socket = sock_dir.join("hush.sock");
    let listener = server::bind_listener(&socket).unwrap();
    assert_eq!(fs::metadata(&sock_dir).unwrap().mode() & 0o777, 0o755);
    drop(listener);
}

#[test]
fn bws_ancestor_group_writable_is_refused() {
    let entries = [
        (0u32, 0o755u32),
        (0u32, 0o775u32),
        (0u32, 0o755u32),
        (0u32, 0o755u32),
    ];
    assert!(paths::check_trusted_components(&entries, 0, false).is_err());
    assert!(paths::check_trusted_components(&entries, 501, true).is_err());
}

#[test]
fn bws_ancestor_non_root_is_refused() {
    let entries = [(0u32, 0o755u32), (501u32, 0o755u32), (0u32, 0o755u32)];
    assert!(paths::check_trusted_components(&entries, 0, false).is_err());
    assert!(paths::check_trusted_components(&entries, 999, true).is_err());
    let ok = [(501u32, 0o755u32), (501u32, 0o755u32)];
    assert!(paths::check_trusted_components(&ok, 501, true).is_ok());
    assert!(paths::check_trusted_components(&ok, 501, false).is_err());
}

#[test]
fn dev_same_user_accepts_root_owned_ancestors() {
    let entries = [(0u32, 0o755u32), (501u32, 0o755u32), (501u32, 0o755u32)];
    assert!(paths::check_trusted_components(&entries, 501, true).is_ok());
    assert!(paths::check_trusted_components(&entries, 501, false).is_err());
}

#[test]
fn non_root_euid_is_refused_without_dev_allowance() {
    assert!(daemon::check_daemon_euid(501, false).is_err());
    assert!(daemon::check_daemon_euid(501, true).is_ok());
    assert!(daemon::check_daemon_euid(0, false).is_ok());
}

#[test]
fn bws_hanging_after_little_output_is_killed_at_deadline() {
    let dir = tempdir("bws-timeout");
    let script = dir.join("bws");
    fs::write(&script, "#!/bin/sh\nprintf hello\nsleep 30\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let backend = backend::BwsBackend::new(script, secrecy::SecretString::from("t"));
    let started = std::time::Instant::now();
    let error = backend
        .run_bws_with_deadline(
            &["secret", "list"],
            1024,
            std::time::Duration::from_millis(300),
        )
        .unwrap_err();
    let text = format!("{error:#}");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(text.contains("timed out"), "unexpected error: {text}");
    assert!(
        !text.contains("hello"),
        "error carried output bytes: {text}"
    );
}

#[test]
fn liveness_probe_reads_a_missing_path_as_stale() {
    let dir = tempdir("liveness-missing");
    let outcome = server::socket_has_liveness(&dir.join("absent.sock"));
    assert!(matches!(outcome, Ok(server::Liveness::Stale)));
    let plain = dir.join("plain.sock");
    fs::write(&plain, "not a socket").unwrap();
    let outcome = server::socket_has_liveness(&plain);
    assert!(matches!(outcome, Ok(server::Liveness::Stale)));
}
