//! The daemon audit log sits below a parent others may write, such as Ubuntu's
//! root:syslog 0775 /var/log. Those group members can rename the log dir or put
//! a symlink in its place. The daemon opens its log dir and file by descriptor
//! without following a symlink, and refuses a log dir anyone else could read or write.

use super::audit::FileAuditLog;
use super::private_temp_harness::PrivateTemp;
use std::fs;
use std::os::unix::fs::{chown, symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// A scratch tree whose `log` dir is writable by the test user's group, like /var/log.
fn shared_log_parent(tag: &str) -> (PrivateTemp, PathBuf) {
    let temp = PrivateTemp::new(tag);
    let log = temp.path().join("log");
    mkdir(&log, 0o775);
    (temp, log)
}

fn mkdir(path: &Path, mode: u32) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().mode() & 0o7777
}

#[test]
fn a_missing_log_dir_below_a_shared_parent_is_created_private() {
    let (_temp, log) = shared_log_parent("audit-shared-parent");
    let path = log.join("hush/audit.log");
    FileAuditLog::open(&path).unwrap();
    assert_eq!(mode(&log.join("hush")), 0o700);
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_log_dir_swapped_for_a_symlink_is_refused_without_writing_through_it() {
    let (temp, log) = shared_log_parent("audit-dir-symlink");
    let elsewhere = temp.path().join("elsewhere");
    mkdir(&elsewhere, 0o700);
    symlink(&elsewhere, log.join("hush")).unwrap();
    assert!(FileAuditLog::open(&log.join("hush/audit.log")).is_err());
    assert!(!elsewhere.join("audit.log").exists());
}

#[test]
fn an_audit_file_that_is_a_symlink_is_refused_and_its_target_kept() {
    let (temp, log) = shared_log_parent("audit-file-symlink");
    mkdir(&log.join("hush"), 0o700);
    let target = temp.path().join("target");
    fs::write(&target, "kept\n").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, log.join("hush/audit.log")).unwrap();
    assert!(FileAuditLog::open(&log.join("hush/audit.log")).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "kept\n");
}

#[test]
fn a_log_dir_put_in_place_by_someone_else_is_refused() {
    let (_temp, log) = shared_log_parent("audit-dir-swapped");
    let hush = log.join("hush");
    mkdir(&hush, 0o770);
    assert!(FileAuditLog::open(&hush.join("audit.log")).is_err());
    assert!(!hush.join("audit.log").exists());
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        fs::set_permissions(&hush, fs::Permissions::from_mode(0o700)).unwrap();
        chown(&hush, Some(1), None).unwrap();
        assert!(FileAuditLog::open(&hush.join("audit.log")).is_err());
        assert!(!hush.join("audit.log").exists());
    }
}

#[test]
fn a_log_dir_owned_by_root_is_refused_by_another_daemon_user_before_any_create() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let path = Path::new("/usr/share/hush-audit-owner-test.log");
    let error = FileAuditLog::open(path).err().unwrap();
    assert!(format!("{error:#}").contains("owned by the daemon user"));
    assert!(!path.exists());
}

#[test]
fn a_log_dir_others_can_read_is_refused_and_an_owner_only_one_is_used() {
    let (_temp, log) = shared_log_parent("audit-dir-readable");
    let hush = log.join("hush");
    let path = hush.join("audit.log");
    for open in [0o755, 0o750, 0o705] {
        mkdir_or_chmod(&hush, open);
        let error = FileAuditLog::open(&path).err().unwrap();
        assert!(format!("{error:#}").contains("owner-only"), "{open:o}");
        assert!(!path.exists());
    }
    mkdir_or_chmod(&hush, 0o700);
    FileAuditLog::open(&path).unwrap();
    assert_eq!(mode(&path), 0o600);
}

fn mkdir_or_chmod(path: &Path, mode: u32) {
    if !path.exists() {
        fs::create_dir(path).unwrap();
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
