//! Publication on real files in a private tree owned by the test user.

use super::custody::{Dir, Owner, Walk, STAGE_PREFIX};
use super::publish::{create_dir, finish_stage, publish};
use super::Fact;
use crate::broker::private_temp_harness::PrivateTemp;
use anyhow::bail;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

fn owner() -> Owner {
    // SAFETY: geteuid takes no arguments and cannot fail.
    Owner(unsafe { libc::geteuid() })
}

fn tree() -> (PrivateTemp, Dir) {
    let temp = PrivateTemp::new("hush-publish-test");
    let Fact::Known(root) = Dir::open_root(temp.path(), owner()) else {
        panic!("private temp root refused");
    };
    (temp, root)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn mode_links(path: &Path) -> (u32, u64) {
    let metadata = fs::symlink_metadata(path).unwrap();
    (metadata.mode() & 0o7777, metadata.nlink())
}

#[test]
fn publish_writes_a_complete_file_once_and_never_replaces_one() {
    let (temp, root) = tree();
    publish(
        &root,
        "unit",
        0o644,
        |file| Ok(file.write_all(b"rendered")?),
    )
    .unwrap();
    assert_eq!(fs::read(temp.path().join("unit")).unwrap(), b"rendered");
    assert_eq!(mode_links(&temp.path().join("unit")), (0o644, 1));
    assert!(publish(&root, "unit", 0o644, |file| Ok(file.write_all(b"other")?)).is_err());
    assert_eq!(fs::read(temp.path().join("unit")).unwrap(), b"rendered");
    assert_eq!(names(temp.path()), ["unit"]);
}

#[test]
fn a_failed_write_publishes_nothing_and_leaves_no_stage() {
    let (temp, root) = tree();
    let failed = publish(&root, "token", 0o600, |file| {
        file.write_all(b"partial")?;
        bail!("input ended early")
    });
    assert!(failed.is_err());
    assert!(names(temp.path()).is_empty());
}

#[test]
fn a_final_name_that_appears_first_is_preserved() {
    let (temp, root) = tree();
    let foreign = temp.path().join("token");
    let refused = publish(&root, "token", 0o600, |file| {
        fs::write(&foreign, b"foreign").unwrap();
        Ok(file.write_all(b"ours")?)
    });
    assert!(refused.is_err());
    assert_eq!(fs::read(&foreign).unwrap(), b"foreign");
    assert_eq!(names(temp.path()), ["token"]);
}

#[test]
fn a_leftover_own_stage_link_is_finished_and_a_foreign_one_is_kept() {
    let (temp, root) = tree();
    let stage = format!("{STAGE_PREFIX}token.41.7");
    fs::write(temp.path().join(&stage), b"ours").unwrap();
    fs::hard_link(temp.path().join(&stage), temp.path().join("token")).unwrap();
    finish_stage(&root, "token", &stage).unwrap();
    assert_eq!(names(temp.path()), ["token"]);
    assert_eq!(mode_links(&temp.path().join("token")).1, 1);
    let foreign = format!("{STAGE_PREFIX}unit.41.7");
    fs::write(temp.path().join(&foreign), b"not the same inode").unwrap();
    fs::write(temp.path().join("unit"), b"unit").unwrap();
    assert!(finish_stage(&root, "unit", &foreign).is_err());
    assert_eq!(names(temp.path()), [foreign.as_str(), "token", "unit"]);
}

#[test]
fn create_dir_makes_an_exact_mode_and_refuses_an_existing_name() {
    let (temp, root) = tree();
    create_dir(&root, "hush", 0o700).unwrap();
    create_dir(&root, "socket", 0o755).unwrap();
    assert_eq!(mode_links(&temp.path().join("hush")).0, 0o700);
    assert_eq!(mode_links(&temp.path().join("socket")).0, 0o755);
    fs::set_permissions(temp.path().join("hush"), fs::Permissions::from_mode(0o750)).unwrap();
    assert!(create_dir(&root, "hush", 0o700).is_err());
    assert_eq!(mode_links(&temp.path().join("hush")).0, 0o750);
    assert!(matches!(
        root.walk(Path::new("socket"), owner()),
        Fact::Known(Walk::Found(_))
    ));
}

#[test]
fn create_dir_below_a_strict_parent_keeps_its_exact_mode_under_a_restrictive_umask() {
    let (temp, root) = tree();
    // SAFETY: umask takes one mode value and returns the previous mask.
    let old = unsafe { libc::umask(0o027) };
    let created = (
        create_dir(&root, "hush", 0o700),
        create_dir(&root, "socket", 0o755),
    );
    // SAFETY: umask takes one mode value and returns the previous mask.
    unsafe { libc::umask(old) };
    created.0.unwrap();
    created.1.unwrap();
    assert_eq!(mode_links(&temp.path().join("hush")).0, 0o700);
    assert_eq!(mode_links(&temp.path().join("socket")).0, 0o755);
}

#[test]
fn create_dir_below_a_shared_parent_never_changes_the_mode_it_made() {
    let (temp, root) = tree();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o770)).unwrap();
    // SAFETY: umask takes one mode value and returns the previous mask.
    let old = unsafe { libc::umask(0o027) };
    let created = (
        create_dir(&root, "hush", 0o700),
        create_dir(&root, "socket", 0o755),
    );
    // SAFETY: umask takes one mode value and returns the previous mask.
    unsafe { libc::umask(old) };
    created.0.unwrap();
    assert!(created.1.is_err());
    assert_eq!(mode_links(&temp.path().join("hush")).0, 0o700);
    assert_eq!(mode_links(&temp.path().join("socket")).0, 0o750);
}
