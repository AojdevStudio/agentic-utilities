//! Regression tests for the broker hardening round.

use super::*;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-fix-test-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fake_backend() -> FakeBackend {
    let mut secrets = HashMap::new();
    secrets.insert(
        "s1".to_string(),
        (
            meta::SecretMeta {
                id: "s1".into(),
                key: "TOKEN".into(),
                project: Some("API Keys".into()),
                project_id: Some("p1".into()),
                note: "scope: test".into(),
            },
            "fake-value".to_string(),
        ),
    );
    FakeBackend::new(secrets)
}

#[test]
fn token_symlink_is_refused() {
    let dir = tempdir("token-symlink");
    let real = dir.join("real-token");
    let link = dir.join("token-link");
    fs::write(&real, "tok\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(token::load_checked(&link, true).is_err());
}

#[test]
fn token_non_root_owner_needs_dev_flag() {
    let euid = unsafe { libc::geteuid() };
    assert!(hardening::check_token_metadata(0o600, 0, euid, false).is_ok());
    assert!(hardening::check_token_metadata(0o600, 1234, euid, false).is_err());
    assert!(hardening::check_token_metadata(0o600, euid, euid, true).is_ok());
    assert!(hardening::check_token_metadata(0o600, 1234, euid, true).is_err());
    let dir = tempdir("token-owner");
    let path = dir.join("token");
    fs::write(&path, "tok\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    if euid != 0 {
        assert!(token::load_checked(&path, false).is_err());
        assert!(token::load_checked(&path, true).is_ok());
    }
}

#[test]
fn socket_modes_ignore_umask() {
    let dir = tempdir("socket-modes");
    let sock_dir = dir.join("run");
    let socket = sock_dir.join("hush.sock");
    // SAFETY: umask takes one mode value and returns the previous mask.
    let old = unsafe { libc::umask(0o077) };
    let listener = server::bind_listener(&socket);
    // SAFETY: umask takes one mode value and returns the previous mask.
    unsafe { libc::umask(old) };
    let listener = listener.unwrap();
    assert_eq!(
        fs::metadata(&sock_dir).unwrap().mode() & 0o777,
        server::SOCKET_DIR_MODE
    );
    assert_eq!(
        fs::metadata(&socket).unwrap().mode() & 0o777,
        server::SOCKET_MODE
    );
    drop(listener);
}

#[test]
fn bind_refuses_regular_file_at_socket_path() {
    let dir = tempdir("socket-file");
    let socket = dir.join("hush.sock");
    fs::write(&socket, "plain file").unwrap();
    assert!(server::bind_listener(&socket).is_err());
    assert_eq!(fs::read_to_string(&socket).unwrap(), "plain file");
}

#[test]
fn stalled_client_does_not_block_second_client() {
    let dir = tempdir("stall");
    let socket = dir.join("hush.sock");
    let audit_path = dir.join("audit.log");
    let audit: Arc<dyn audit::AuditLog> = Arc::new(audit::FileAuditLog::open(&audit_path).unwrap());
    let backend: Arc<dyn VaultBackend> = Arc::new(fake_backend());
    let listener = server::bind_listener(&socket).unwrap();
    let config = daemon::ServeConfig {
        read_timeout: std::time::Duration::from_secs(10),
        write_timeout: std::time::Duration::from_secs(10),
        max_connections: 32,
    };
    let _server = std::thread::spawn(move || {
        let _ = server::run_server(listener, backend, audit, config);
    });
    let mut stalled = UnixStream::connect(&socket).unwrap();
    stalled.write_all(&[1u8, 0u8]).unwrap();
    let stream2 = UnixStream::connect(&socket).unwrap();
    stream2
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    let mut client2 = client::Client::from_stream(stream2).unwrap();
    let result = client2.catalog(None);
    assert!(result.is_ok(), "second client was blocked: {result:?}");
}

#[test]
fn oversized_bws_output_is_refused_without_bytes_in_error() {
    let dir = tempdir("big-bws");
    let script = dir.join("bws");
    fs::write(
        &script,
        "#!/bin/sh\nhead -c 20000000 /dev/zero | tr '\\0' 'A'\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let backend = backend::BwsBackend::new(script, secrecy::SecretString::from("t"));
    let error = backend
        .run_bws_capped(&["secret", "list"], 1024)
        .unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("size limit"), "unexpected error: {text}");
    assert!(!text.contains("AAAA"), "error carried output bytes: {text}");
}

#[test]
fn untrusted_bws_path_is_refused() {
    assert!(paths::validate_trusted_bws_path(std::path::Path::new("bws"), true).is_err());
    let dir = tempdir("bws-group");
    let script = dir.join("bws");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o775)).unwrap();
    assert!(paths::validate_trusted_bws_path(&script, true).is_err());
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o775)).unwrap();
    assert!(paths::validate_trusted_bws_path(&script, true).is_err());
}

#[test]
fn trusted_bws_path_checks_are_pure() {
    let euid = unsafe { libc::geteuid() };
    assert!(hardening::check_owner(0, euid, false).is_ok());
    assert!(hardening::check_owner(1234, euid, false).is_err());
    assert!(hardening::check_owner(euid, euid, true).is_ok());
    assert!(hardening::check_not_group_or_world_writable(0o755).is_ok());
    assert!(hardening::check_not_group_or_world_writable(0o775).is_err());
    assert!(hardening::check_not_group_or_world_writable(0o757).is_err());
}

#[test]
fn connected_daemon_error_propagates_as_error() {
    let dir = tempdir("daemon-error");
    let socket = dir.join("hush.sock");
    let audit_path = dir.join("audit.log");
    let audit: Arc<dyn audit::AuditLog> = Arc::new(audit::FileAuditLog::open(&audit_path).unwrap());
    let backend: Arc<dyn VaultBackend> = Arc::new(backend::FailingBackend);
    let listener = server::bind_listener(&socket).unwrap();
    let _server = std::thread::spawn(move || {
        let _ = server::run_server(listener, backend, audit, daemon::ServeConfig::default());
    });
    let outcome = client::Client::try_connect_to(&socket).unwrap();
    let ConnectOutcome::Connected(mut connected) = outcome else {
        panic!("expected a connected client");
    };
    let result = connected.catalog(None);
    assert!(result.is_err(), "connected daemon error was swallowed");
}

#[test]
fn missing_socket_and_plain_file_report_unavailable() {
    let dir = tempdir("unavailable");
    let missing = dir.join("missing.sock");
    let outcome = client::Client::try_connect_to(&missing).unwrap();
    assert!(matches!(outcome, ConnectOutcome::Unavailable));
    let plain = dir.join("plain.sock");
    fs::write(&plain, "not a socket").unwrap();
    let outcome = client::Client::try_connect_to(&plain).unwrap();
    assert!(matches!(outcome, ConnectOutcome::Unavailable));
}
