//! Regression tests for the broker hardening round 2.

use super::*;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-round2-test-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // A loose umask (002 on the CI box) would make this group-writable, which the socket check refuses.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
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

/// A saturated accept queue must never end with the socket unlinked. The
/// filler connects nonblocking with a per-attempt deadline, so saturating a
/// full queue can never hang the test. When saturation is observed, the pending
/// attempt returns refused; on Linux the backlog cap may never fill, so the
/// duplicate-bind refusal and socket survival are the platform-proof asserts.
#[test]
fn saturated_listener_socket_is_never_unlinked() {
    let dir = tempdir("saturated");
    let socket = dir.join("hush.sock");
    let listener = server::bind_listener(&socket).unwrap();

    // Each enqueued connection holds its slot after the client closes, so the
    // helper closes every stream as soon as the kernel enqueues it. Both
    // platforms bound the queue (somaxconn on Linux, sonewconn on macOS), so
    // the filler stops at the first refusal and asserts it happened.
    let mut saturated = false;
    for _ in 0..65536 {
        match connect_with_deadline(&socket, 200) {
            Ok(_stream) => {}
            Err(ConnectMode::RefusedOrFull) => {
                saturated = true;
                break;
            }
            Err(ConnectMode::Transient) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
    assert!(saturated, "the accept queue did not fill within the bound");

    // The duplicate bind must be refused and the socket kept, saturated or not.
    let second = server::bind_listener(&socket);
    assert!(second.is_err(), "bind over the live socket should refuse");
    assert!(
        fs::symlink_metadata(&socket).is_ok(),
        "socket file was removed while the listener is still bound"
    );

    // Drain: dead entries only leave when the listener accepts them.
    std::thread::sleep(std::time::Duration::from_millis(100));
    listener.set_nonblocking(true).unwrap();
    let mut reconnected = false;
    for _ in 0..40 {
        while let Ok((_, _conn)) = listener.accept() {}
        if UnixStream::connect(&socket).is_ok() {
            reconnected = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        reconnected,
        "the original listener stayed unreachable after draining"
    );
    drop(listener);
}

enum ConnectMode {
    /// The kernel refused or rejected the attempt; the queue is not accepting.
    RefusedOrFull,
    /// The attempt is still pending or could not start; retry.
    Transient,
}

/// A helper for attempts: nonblocking connect polled to a deadline;
/// each attempt is bounded, so a saturated queue cannot hang the loop.
fn connect_with_deadline(path: &Path, deadline_ms: i32) -> Result<UnixStream, ConnectMode> {
    // SAFETY: fd closes on every path, sockaddr from a zeroed struct.
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
        if fd < 0 {
            return Err(ConnectMode::Transient);
        }
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            libc::close(fd);
            return Err(ConnectMode::Transient);
        }
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() >= addr.sun_path.len() {
            libc::close(fd);
            return Err(ConnectMode::Transient);
        }
        addr.sun_path[..bytes.len()].copy_from_slice(std::mem::transmute::<&[u8], &[i8]>(bytes));
        let connected = libc::connect(
            fd,
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        );
        if connected == 0 {
            let stream = UnixStream::from_raw_fd(fd);
            stream.set_nonblocking(false).ok();
            return Ok(stream);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINPROGRESS | libc::EAGAIN | libc::EINTR) => {
                let mut pollfd = libc::pollfd {
                    fd,
                    events: libc::POLLHUP,
                    revents: 0,
                };
                let waited = libc::poll(&mut pollfd, 1, deadline_ms);
                if waited < 0 {
                    libc::close(fd);
                    return Err(ConnectMode::Transient);
                }
                if pollfd.revents & libc::POLLERR != 0 {
                    libc::close(fd);
                    return Err(ConnectMode::RefusedOrFull);
                }
                if pollfd.revents & libc::POLLHUP != 0 {
                    libc::close(fd);
                    return Err(ConnectMode::RefusedOrFull);
                }
                libc::close(fd);
                Err(ConnectMode::Transient)
            }
            Some(libc::ECONNREFUSED) => {
                libc::close(fd);
                Err(ConnectMode::RefusedOrFull)
            }
            _ => {
                libc::close(fd);
                Err(ConnectMode::Transient)
            }
        }
    }
}
