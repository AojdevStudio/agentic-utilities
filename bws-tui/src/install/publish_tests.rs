//! Publication windows on real trees: a crash leftover, a racing writer, and
//! a target that differs. Nothing existing is replaced.

use super::layout::{Kind, Spec};
use super::publish::publish;
use super::tree::Tree;
use std::io::Write;
use std::path::PathBuf;

fn spec() -> Spec {
    Spec { path: PathBuf::from("/etc/hush/unit"), kind: Kind::File(b"want\n".to_vec()) }
}

fn tree(tag: &str) -> Tree {
    let tree = Tree::new(tag);
    tree.dirs("/etc/hush");
    tree
}

fn stages(tree: &Tree) -> Vec<String> {
    std::fs::read_dir(tree.path("/etc/hush"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("hush-stage"))
        .collect()
}

#[test]
fn a_crash_leftover_stage_does_not_block_publication() {
    let tree = tree("publish-leftover");
    let leftover = tree.file("/etc/hush/.unit.hush-stage-1-1", b"partial", 0o600);
    publish(&tree.layout, &spec(), |file| Ok(file.write_all(b"want\n")?)).unwrap();
    assert_eq!(std::fs::read(tree.path("/etc/hush/unit")).unwrap(), b"want\n");
    assert_eq!(std::fs::read(&leftover).unwrap(), b"partial", "a foreign stage is never touched");
}

#[test]
fn a_matching_racer_counts_as_done_and_a_different_one_is_kept() {
    let tree = tree("publish-race");
    let racer = tree.path("/etc/hush/unit");
    publish(&tree.layout, &spec(), |file| {
        std::fs::write(&racer, b"want\n")?;
        std::fs::set_permissions(&racer, std::os::unix::fs::PermissionsExt::from_mode(0o644))?;
        Ok(file.write_all(b"want\n")?)
    })
    .unwrap();
    assert!(stages(&tree).is_empty());

    let tree = self::tree("publish-differs");
    let racer = tree.path("/etc/hush/unit");
    let result = publish(&tree.layout, &spec(), |file| {
        std::fs::write(&racer, b"theirs\n")?;
        Ok(file.write_all(b"want\n")?)
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&racer).unwrap(), b"theirs\n");
    assert!(stages(&tree).is_empty(), "the failed stage is removed");
}

#[test]
fn a_failed_fill_leaves_no_target_and_no_stage() {
    let tree = tree("publish-fill");
    let result = publish(&tree.layout, &spec(), |_| anyhow::bail!("helper failed"));
    assert!(result.is_err());
    assert!(!tree.path("/etc/hush/unit").exists());
    assert!(stages(&tree).is_empty());
}

#[test]
fn a_swapped_parent_is_refused_before_staging() {
    let tree = tree("publish-swap");
    std::fs::rename(tree.path("/etc/hush"), tree.path("/etc/real")).unwrap();
    std::os::unix::fs::symlink(tree.path("/etc/real"), tree.path("/etc/hush")).unwrap();
    assert!(publish(&tree.layout, &spec(), |file| Ok(file.write_all(b"want\n")?)).is_err());
    assert_eq!(std::fs::read_dir(tree.path("/etc/real")).unwrap().count(), 0);
}
