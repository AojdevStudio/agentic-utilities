//! Which daemon a client hands its terminal and stdin to. A release client
//! trusts only root and the fixed socket path. Same-user daemons and the
//! `HUSH_SOCKET` override exist for debug builds and tests.

use super::client_human::trusts_daemon;
use super::paths::{socket_path, Build};
use std::path::PathBuf;

#[test]
fn release_clients_pass_descriptors_only_to_a_root_daemon() {
    assert!(trusts_daemon(0, 501, Build::Release));
    assert!(!trusts_daemon(501, 501, Build::Release), "same user");
    assert!(!trusts_daemon(502, 501, Build::Release), "other user");
    assert!(trusts_daemon(0, 501, Build::Debug));
    assert!(trusts_daemon(501, 501, Build::Debug), "debug same user");
    assert!(!trusts_daemon(502, 501, Build::Debug), "debug other user");
}

#[test]
fn release_clients_ignore_the_socket_override() {
    let fixed = socket_path(None, Build::Release);
    assert_eq!(
        socket_path(Some("/tmp/fake.sock".into()), Build::Release),
        fixed
    );
    assert_eq!(
        socket_path(Some("/tmp/fake.sock".into()), Build::Debug),
        PathBuf::from("/tmp/fake.sock")
    );
    assert_eq!(socket_path(None, Build::Debug), fixed);
}
