//! What a `bws` child can hold, and what the daemon keeps after it. A
//! timed-out call leaves no reader behind, and on macOS no stray daemon
//! descriptor reaches the child.

use super::backend::BwsBackend;
use super::run_harness::tempdir;
use secrecy::SecretString;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

fn script(dir: &Path, body: &str) -> BwsBackend {
    let path = dir.join("bws");
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    BwsBackend::new(path, SecretString::from("t"))
}

/// A grandchild keeps the output pipe open after the daemon gives up. Once the
/// call returns, the daemon must hold no read end, so the late write fails.
#[test]
fn a_timed_out_bws_call_leaves_no_reader_on_its_output() {
    let dir = tempdir("bws-no-reader");
    let marker = dir.join("late-write-landed");
    let body = format!(
        "( sleep 1; printf late && : > '{}' ) &\nsleep 30\n",
        marker.display()
    );
    let backend = script(&dir, &body);
    let timeout = Duration::from_millis(300);
    let failed = backend.run_bws_with_deadline(&["secret", "list"], 1024, timeout);
    assert!(failed.is_err(), "the call should time out");
    assert_eq!(
        backend.busy_children(),
        0,
        "the killed bws child was not reaped"
    );
    std::thread::sleep(Duration::from_millis(1800));
    assert!(!marker.exists(), "a reader still held the bws output pipe");
}

/// macOS receives passed descriptors without close-on-exec and marks them one
/// call later. The child must get only its stdio either way.
#[cfg(target_os = "macos")]
#[test]
fn a_bws_child_inherits_no_stray_descriptor() {
    use std::os::fd::AsRawFd;
    let dir = tempdir("bws-no-stray-fd");
    let (reader, _writer) = super::fdpass_tests::pipe();
    // SAFETY: dup returns a new descriptor without close-on-exec, owned below.
    let stray = unsafe { libc::dup(reader.as_raw_fd()) };
    assert!(stray > 2);
    let marker = dir.join("inherited");
    let body = format!(
        "if [ -e /dev/fd/{stray} ]; then : > '{}'; fi\nprintf '[]'\n",
        marker.display()
    );
    let backend = script(&dir, &body);
    let listed = backend.run_bws_with_deadline(&["project", "list"], 1024, Duration::from_secs(10));
    // SAFETY: closes the descriptor this test duplicated.
    unsafe { libc::close(stray) };
    assert_eq!(listed.unwrap().as_str(), "[]");
    assert!(!marker.exists(), "fd {stray} reached the bws child");
}
