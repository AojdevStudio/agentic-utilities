//! Native helper selection on real protected filesystem layouts. No helper runs.

use super::custody::Dir;
use super::fixture_harness::{owner, Fixture};
use super::layout::Platform;
use super::native_program::checked;
use super::Fact;
use std::fs;
use std::os::unix::fs::symlink;

type Change = fn(&Fixture);

fn root(fixture: &Fixture) -> Dir {
    let Fact::Known(root) = Dir::open_root(fixture.temp.path(), owner()) else {
        panic!("fixture root refused");
    };
    root
}

fn merged() -> Fixture {
    let fixture = Fixture::new(Platform::Linux);
    fs::remove_file(fixture.path("usr/sbin/nologin")).unwrap();
    fs::remove_dir(fixture.path("usr/sbin")).unwrap();
    fixture.mkdir("usr/bin", 0o755);
    fixture.put("usr/bin/useradd", b"synthetic, never executed\n", 0o755);
    symlink("bin", fixture.path("usr/sbin")).unwrap();
    fixture
}

#[test]
fn native_useradd_selection_returns_the_checked_canonical_executable() {
    let plain = Fixture::new(Platform::Linux);
    plain.put("usr/sbin/useradd", b"synthetic, never executed\n", 0o755);
    let merged = merged();
    for (fixture, expected) in [(&plain, "usr/sbin/useradd"), (&merged, "usr/bin/useradd")] {
        let before = fixture.snapshot();
        let selected = checked(&root(fixture), owner(), "/usr/sbin/useradd");
        assert!(
            selected.is_ok(),
            "protected native useradd refused: {selected:?}"
        );
        assert_eq!(selected.unwrap(), fixture.path(expected));
        assert_eq!(
            fixture.snapshot(),
            before,
            "selection changed the filesystem"
        );
    }
}

#[test]
fn canonical_native_helpers_keep_the_existing_custody_checks() {
    let cases: [(&str, Change); 4] = [
        ("writable parent", |f| f.mkdir_mode("usr/bin", 0o777)),
        ("writable helper", |f| {
            f.mkdir_mode("usr/bin/useradd", 0o775)
        }),
        ("hard-linked helper", |f| {
            fs::hard_link(f.path("usr/bin/useradd"), f.path("usr/bin/other")).unwrap();
        }),
        ("ACL grant", |f| {
            super::input_tests::grant(&f.path("usr/bin/useradd"), 0o755);
        }),
    ];
    for (name, change) in cases {
        let fixture = merged();
        change(&fixture);
        let before = fixture.snapshot();
        assert!(
            checked(&root(&fixture), owner(), "/usr/sbin/useradd").is_err(),
            "{name} was accepted"
        );
        assert_eq!(fixture.snapshot(), before, "{name} changed the filesystem");
    }
}
