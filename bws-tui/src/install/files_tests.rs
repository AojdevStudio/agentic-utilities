//! Real filesystem custody checks on private trees.

use super::files::{check_executable, inspect};
use super::layout::{Kind, Spec};
use super::model::{Backend, Fact, Presence};
use super::tree::Tree;
use std::path::PathBuf;

fn token_spec() -> Spec {
    Spec {
        path: PathBuf::from("/etc/hush/token"),
        kind: Kind::Token(Backend::LoadCredential),
    }
}

#[test]
fn a_ready_token_source_passes_its_checks() {
    let tree = Tree::new("files-ready");
    tree.file("/etc/hush/token", b"synthetic", 0o600);
    tree.chmod("/etc/hush", 0o700);
    assert_eq!(inspect(&tree.layout, &token_spec(), true), Fact::Known(Presence::Ready));
}

#[test]
fn a_parent_writable_by_others_is_refused() {
    let tree = Tree::new("files-writable");
    tree.file("/etc/hush/token", b"synthetic", 0o600);
    tree.chmod("/etc", 0o775);
    assert!(matches!(inspect(&tree.layout, &token_spec(), true), Fact::Refused(_)));
    tree.chmod("/etc", 0o757);
    assert!(matches!(inspect(&tree.layout, &token_spec(), true), Fact::Refused(_)));
}

#[test]
fn symlinks_special_files_and_links_are_refused() {
    let tree = Tree::new("files-shapes");
    let real = tree.file("/etc/other/token", b"synthetic", 0o600);
    tree.dirs("/etc/hush");
    std::os::unix::fs::symlink(&real, tree.path("/etc/hush/token")).unwrap();
    assert!(matches!(inspect(&tree.layout, &token_spec(), true), Fact::Refused(_)));
    std::fs::remove_file(tree.path("/etc/hush/token")).unwrap();
    std::fs::hard_link(&real, tree.path("/etc/hush/token")).unwrap();
    assert_eq!(
        inspect(&tree.layout, &token_spec(), true),
        Fact::Refused("has another hard link")
    );
    std::fs::remove_file(tree.path("/etc/hush/token")).unwrap();
    let raw = std::ffi::CString::new(tree.path("/etc/hush/token").to_str().unwrap()).unwrap();
    // SAFETY: mkfifo reads one valid C path.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    assert!(matches!(inspect(&tree.layout, &token_spec(), true), Fact::Refused(_)));
    std::fs::remove_file(tree.path("/etc/hush/token")).unwrap();
    std::fs::rename(tree.path("/etc/hush"), tree.path("/etc/moved")).unwrap();
    std::os::unix::fs::symlink(tree.path("/etc/moved"), tree.path("/etc/hush")).unwrap();
    tree.file("/etc/moved/token", b"synthetic", 0o600);
    assert!(matches!(inspect(&tree.layout, &token_spec(), true), Fact::Refused(_)));
}

#[test]
fn a_missing_token_is_absent_only_below_a_managed_parent() {
    let tree = Tree::new("files-absent");
    tree.dirs("/etc");
    assert_eq!(inspect(&tree.layout, &token_spec(), true), Fact::Known(Presence::Absent));
    assert!(matches!(inspect(&tree.layout, &token_spec(), false), Fact::Refused(_)));
}

#[cfg(target_os = "linux")]
#[test]
fn an_access_list_grant_is_refused() {
    let tree = Tree::new("files-acl");
    let path = tree.file("/etc/hush/token", b"synthetic", 0o600);
    tree.chmod("/etc/hush", 0o700);
    let status = std::process::Command::new("setfacl")
        .args(["-m", "u:nobody:r"])
        .arg(&path)
        .status()
        .expect("setfacl is installed");
    assert!(status.success());
    tree.chmod("/etc/hush/token", 0o600);
    assert_eq!(
        inspect(&tree.layout, &token_spec(), true),
        Fact::Refused("has an access list grant")
    );
}

#[test]
fn executables_need_protected_custody() {
    let tree = Tree::new("files-exec");
    tree.file("/usr/local/bin/hush", b"#!/bin/false\n", 0o755);
    let exe = PathBuf::from("/usr/local/bin/hush");
    assert_eq!(check_executable(&tree.layout, &exe), Fact::Known(()));
    for mode in [0o775, 0o4755, 0o644] {
        tree.chmod(&exe, mode);
        assert!(matches!(check_executable(&tree.layout, &exe), Fact::Refused(_)), "{mode:o}");
    }
    tree.chmod(&exe, 0o755);
    tree.chmod("/usr/local", 0o777);
    assert!(matches!(check_executable(&tree.layout, &exe), Fact::Refused(_)));
}
