//! Custody checks on real files in a private tree owned by the test user.

use super::custody::{Contents, Dir, FileState, Mode, Owner, Walk};
use super::Fact;
use crate::broker::private_temp_harness::PrivateTemp;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

fn owner() -> Owner {
    // SAFETY: geteuid takes no arguments and cannot fail.
    Owner(unsafe { libc::geteuid() })
}

fn tree() -> (PrivateTemp, Dir) {
    let temp = PrivateTemp::new("hush-custody-test");
    let Fact::Known(root) = Dir::open_root(temp.path(), owner()) else {
        panic!("private temp root refused");
    };
    (temp, root)
}

fn put(path: &Path, bytes: &[u8], mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

/// A directory at 0700, whatever the umask.
fn mkdir(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn file_state(root: &Dir, name: &str, mode: Mode, contents: Contents<'_>) -> Fact<FileState> {
    let Fact::Known(Walk::Found(dir)) = root.walk(Path::new("etc/hush"), owner()) else {
        panic!("etc/hush refused");
    };
    dir.file(name, mode, owner(), contents)
}

fn refused<T>(fact: Fact<T>) -> bool {
    matches!(fact, Fact::Refused(_))
}

fn private_dirs(temp: &PrivateTemp) -> std::path::PathBuf {
    let dir = temp.path().join("etc/hush");
    fs::create_dir_all(&dir).unwrap();
    for path in [temp.path().join("etc"), dir.clone()] {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

#[test]
fn checked_files_are_ready_absent_or_resumable_from_their_own_stage_link() {
    let (temp, root) = tree();
    let dir = private_dirs(&temp);
    put(&dir.join("token"), b"opaque", 0o600);
    put(&dir.join("unit"), b"rendered", 0o644);
    fs::hard_link(dir.join("unit"), dir.join(".hush-stage.unit.7.9")).unwrap();
    let state = |name, mode: Mode, contents| match file_state(&root, name, mode, contents) {
        Fact::Known(state) => state,
        other => panic!("{name}: {other:?}"),
    };
    assert_eq!(
        state("token", Mode::Exactly(0o600), Contents::Opaque),
        FileState::Ready
    );
    assert_eq!(
        state("absent", Mode::Exactly(0o600), Contents::Opaque),
        FileState::Absent
    );
    assert_eq!(
        state("unit", Mode::Exactly(0o644), Contents::Exactly(b"rendered")),
        FileState::Resume {
            stage: ".hush-stage.unit.7.9".into()
        }
    );
}

#[test]
fn unsafe_or_mismatching_files_are_refused() {
    let (temp, root) = tree();
    let dir = private_dirs(&temp);
    put(&dir.join("target"), b"x", 0o600);
    symlink(dir.join("target"), dir.join("link")).unwrap();
    put(&dir.join("readable"), b"x", 0o640);
    put(&dir.join("setuid"), b"x", 0o4600);
    put(&dir.join("linked"), b"x", 0o600);
    fs::hard_link(dir.join("linked"), dir.join("other-name")).unwrap();
    put(&dir.join("differs"), b"operator edit", 0o644);
    let raw = std::ffi::CString::new(dir.join("fifo").as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: `raw` is a valid NUL-terminated path for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
    for name in ["link", "readable", "setuid", "linked", "fifo"] {
        assert!(
            refused(file_state(
                &root,
                name,
                Mode::Exactly(0o600),
                Contents::Opaque
            )),
            "{name}"
        );
    }
    assert!(refused(file_state(
        &root,
        "differs",
        Mode::Exactly(0o644),
        Contents::Exactly(b"rendered")
    )));
    let Fact::Known(Walk::Found(hush)) = root.walk(Path::new("etc/hush"), owner()) else {
        panic!("etc/hush refused");
    };
    assert!(refused(hush.file(
        "target",
        Mode::Exactly(0o600),
        Owner(owner().0 + 1),
        Contents::Opaque
    )));
}

#[test]
fn unsafe_ancestors_are_refused_and_missing_ones_are_reported() {
    let (temp, root) = tree();
    fs::create_dir_all(temp.path().join("open/inner")).unwrap();
    fs::set_permissions(temp.path().join("open"), fs::Permissions::from_mode(0o777)).unwrap();
    mkdir(&temp.path().join("real"));
    symlink(temp.path().join("real"), temp.path().join("alias")).unwrap();
    fs::create_dir(temp.path().join("locked")).unwrap();
    fs::create_dir(temp.path().join("locked/inner")).unwrap();
    fs::set_permissions(
        temp.path().join("locked"),
        fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    assert!(refused(root.walk(Path::new("open/inner"), owner())));
    assert!(refused(root.walk(Path::new("alias"), owner())));
    let locked = root.walk(Path::new("locked/inner"), owner());
    fs::set_permissions(
        temp.path().join("locked"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert!(
        matches!(locked, Fact::Unknown(_)),
        "unreadable ancestor is not unknown"
    );
    match root.walk(Path::new("real/a/b"), owner()) {
        Fact::Known(Walk::Missing { parent, rest }) => {
            assert_eq!(parent.path(), temp.path().join("real"));
            assert_eq!(rest, ["a", "b"]);
        }
        _ => panic!("missing components not reported"),
    }
}

#[test]
fn extended_acl_grants_are_refused_on_files_and_directories() {
    let (temp, root) = tree();
    let dir = private_dirs(&temp);
    put(&dir.join("token"), b"x", 0o600);
    #[cfg(target_os = "linux")]
    let grant = |path: &Path| {
        Command::new("setfacl")
            .args(["-m", "u:65534:r"])
            .arg(path)
            .status()
    };
    #[cfg(target_os = "macos")]
    let grant = |path: &Path| {
        Command::new("chmod")
            .args(["+a", "everyone allow read"])
            .arg(path)
            .status()
    };
    assert!(grant(&dir.join("token")).unwrap().success());
    assert!(refused(file_state(
        &root,
        "token",
        Mode::Exactly(0o600),
        Contents::Opaque
    )));
    fs::create_dir(temp.path().join("granted")).unwrap();
    assert!(grant(&temp.path().join("granted")).unwrap().success());
    assert!(refused(root.walk(Path::new("granted"), owner())));
}

#[test]
fn executables_need_protected_regular_unshared_executable_files() {
    let (temp, root) = tree();
    let bin = temp.path().join("bin");
    mkdir(&bin);
    for (name, mode) in [
        ("ok", 0o755),
        ("group-write", 0o775),
        ("setuid", 0o4755),
        ("plain", 0o644),
    ] {
        put(&bin.join(name), b"#!/bin/false\n", mode);
    }
    put(&bin.join("shared"), b"#!/bin/false\n", 0o755);
    fs::hard_link(bin.join("shared"), temp.path().join("shared-copy")).unwrap();
    symlink(bin.join("ok"), bin.join("link")).unwrap();
    assert!(matches!(
        root.executable(Path::new("bin/ok"), owner()),
        Fact::Known(())
    ));
    for name in ["group-write", "setuid", "plain", "shared", "link"] {
        assert!(
            refused(root.executable(&Path::new("bin").join(name), owner())),
            "{name}"
        );
    }
}

#[test]
fn protected_and_private_modes_accept_operator_files_and_refuse_shared_ones() {
    let (temp, root) = tree();
    let dir = private_dirs(&temp);
    for (name, mode) in [
        ("pam-0644", 0o644),
        ("pam-0600", 0o600),
        ("pam-0664", 0o664),
        ("pam-setgid", 0o2644),
        ("key-0400", 0o400),
        ("key-0440", 0o440),
    ] {
        put(&dir.join(name), b"x", mode);
    }
    let known = |name, mode| {
        matches!(
            file_state(&root, name, mode, Contents::Opaque),
            Fact::Known(FileState::Ready)
        )
    };
    assert!(known("pam-0644", Mode::Protected) && known("pam-0600", Mode::Protected));
    assert!(!known("pam-0664", Mode::Protected) && !known("pam-setgid", Mode::Protected));
    assert!(known("key-0400", Mode::Private) && known("pam-0600", Mode::Private));
    assert!(!known("key-0440", Mode::Private) && !known("pam-0644", Mode::Private));
}
