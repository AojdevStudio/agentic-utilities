//! Ubuntu ships /var/log as root:syslog 0775. The log dir's root-owned parent
//! may be group writable only for a named system group that a protected
//! etc/group binds to one unique gid. The members that file lists are trusted.
//! Every other shared directory stays refused.

use super::{p, plan, setup};
use crate::install::apply_harness::{install, Token};
use crate::install::fake_host_harness::FakeHost;
use crate::install::fixture_harness::{owner, Fixture};
use crate::install::input_tests::grant;
use crate::install::layout::{Layout, Platform};
use crate::install::observe::observe;
use crate::install::plan::{Plan, Step};
use crate::install::status::Status;
use std::fs;
use std::os::unix::fs::{chown, MetadataExt, PermissionsExt};
use std::path::PathBuf;

const SHARED: &str = "var/log is group or world writable";
const SYSLOG: &str = "root:x:0:\nsyslog:x:GID:\n";

type Change = fn(&Fixture, u32);

/// The test user's own group, or an unused gid when root may chown to any.
fn shared_gid() -> u32 {
    // SAFETY: geteuid and getegid take no arguments and cannot fail.
    unsafe {
        if libc::geteuid() == 0 {
            4242
        } else {
            libc::getegid()
        }
    }
}

fn share(fixture: &Fixture, rel: &str, gid: u32, mode: u32) {
    chown(fixture.path(rel), Some(owner().0), Some(gid)).unwrap();
    fixture.mkdir_mode(rel, mode);
}

/// A Linux tree whose var/log is owner:gid 0775, with `group` as etc/group.
fn shared_log(group: &str) -> (Fixture, FakeHost, u32) {
    let (fixture, host) = setup(Platform::Linux);
    let gid = shared_gid();
    share(&fixture, "var/log", gid, 0o775);
    let group = group.replace("GID", &gid.to_string());
    fixture.put("etc/group", group.as_bytes(), 0o644);
    (fixture, host, gid)
}

#[test]
fn a_log_parent_shared_with_a_protected_system_group_plans_the_log_dir() {
    for name in ["syslog", "adm"] {
        let (fixture, mut host, _) = shared_log(&format!("root:x:0:\n{name}:x:GID:\n"));
        let steps = plan(&fixture, &mut host).unwrap_or_else(|why| panic!("{name}: {why:?}"));
        assert!(
            steps.contains(&Step::CreateDir(p("/var/log/hush"))),
            "{name}: {steps:?}"
        );
    }
}

#[test]
fn a_log_parent_shared_with_a_system_group_that_lists_members_plans_the_log_dir() {
    let (fixture, mut host, _) = shared_log("root:x:0:\nadm:x:GID:syslog,operator\n");
    let steps = plan(&fixture, &mut host).unwrap_or_else(|why| panic!("{why:?}"));
    assert!(
        steps.contains(&Step::CreateDir(p("/var/log/hush"))),
        "{steps:?}"
    );
}

#[test]
fn install_creates_a_private_log_dir_below_a_syslog_shared_var_log() {
    let (fixture, mut host, _) = shared_log(SYSLOG);
    install(&fixture, &mut host, &mut Token::default()).unwrap();
    let mode = fs::symlink_metadata(fixture.path("var/log/hush"))
        .unwrap()
        .mode();
    assert_eq!(mode & 0o7777, 0o700);
    let status = Status(&observe(fixture.layout(), &mut host)).to_string();
    assert!(status.starts_with("installed: yes"), "{status}");
}

#[test]
fn every_other_shared_parent_or_private_dir_is_still_refused() {
    let cases: [(&str, &str, &str, Change); 12] = [
        ("world write", SYSLOG, SHARED, |f, g| {
            share(f, "var/log", g, 0o777)
        }),
        ("other write alone", SYSLOG, SHARED, |f, g| {
            share(f, "var/log", g, 0o757)
        }),
        (
            "untrusted group",
            "root:x:0:\nstaff:x:GID:\n",
            SHARED,
            |_, _| {},
        ),
        (
            "group named twice",
            "syslog:x:GID:\nsyslog:x:4243:\n",
            SHARED,
            |_, _| {},
        ),
        (
            "gid shared by another group",
            "syslog:x:GID:\nlocal:x:GID:\n",
            SHARED,
            |_, _| {},
        ),
        ("unprotected group file", SYSLOG, SHARED, |f, _| {
            fs::set_permissions(f.path("etc/group"), fs::Permissions::from_mode(0o664)).unwrap()
        }),
        ("no group file", SYSLOG, SHARED, |f, _| {
            fs::remove_file(f.path("etc/group")).unwrap()
        }),
        (
            "ACL grant",
            SYSLOG,
            "var/log has extended ACL grants",
            |f, _| grant(&f.path("var/log"), 0o775),
        ),
        (
            "shared private log dir",
            SYSLOG,
            "var/log/hush is group or world writable",
            |f, g| {
                f.mkdir("var/log/hush", 0o770);
                share(f, "var/log/hush", g, 0o770);
            },
        ),
        (
            "shared private token dir",
            SYSLOG,
            "etc/hush is group or world writable",
            |f, g| {
                f.mkdir("etc/hush", 0o770);
                share(f, "etc/hush", g, 0o770);
            },
        ),
        (
            "shared log ancestor",
            SYSLOG,
            "var is group or world writable",
            |f, g| share(f, "var", g, 0o775),
        ),
        (
            "shared binary dir",
            SYSLOG,
            "usr/local/bin is group or world writable",
            |f, g| share(f, "usr/local/bin", g, 0o775),
        ),
    ];
    for (name, group, expected, change) in cases {
        let (fixture, mut host, gid) = shared_log(group);
        change(&fixture, gid);
        let before = fixture.snapshot();
        let reasons = plan(&fixture, &mut host).expect_err(name);
        assert!(
            reasons.iter().any(|reason| reason.contains(expected)),
            "{name}: {reasons:?}"
        );
        assert!(host.actions.is_empty(), "{name}");
        assert_eq!(fixture.snapshot(), before, "{name}");
    }
}

type Place = fn(&mut Layout, PathBuf);
/// A case name, a directory to make with its mode, a program path, and where the layout looks for it.
type Program<'a> = (&'a str, Option<(&'a str, u32)>, &'a str, Place);

#[test]
fn programs_below_a_shared_log_parent_are_refused() {
    let hush: Place = |layout, path| layout.executable = path;
    let bws: Place = |layout, path| layout.bws_candidates = vec![path];
    let cases: [Program; 4] = [
        (
            "hush in a log subdir",
            Some(("var/log/build", 0o755)),
            "/var/log/build/hush",
            hush,
        ),
        ("hush beside the log dir", None, "/var/log/hush-bin", hush),
        (
            "hush in the log dir",
            Some(("var/log/hush", 0o700)),
            "/var/log/hush/hush",
            hush,
        ),
        (
            "bws in a log subdir",
            Some(("var/log/build", 0o755)),
            "/var/log/build/bws",
            bws,
        ),
    ];
    for (name, dir, program, place) in cases {
        let (fixture, mut host, _) = shared_log(SYSLOG);
        if let Some((dir, mode)) = dir {
            fixture.mkdir(dir, mode);
        }
        fixture.put(program, b"fixture\n", 0o755);
        let mut layout = fixture.layout();
        place(&mut layout, p(program));
        let before = fixture.snapshot();
        let reasons = Plan::from_observation(observe(layout, &mut host))
            .map(|plan| plan.steps().to_vec())
            .expect_err(name)
            .0;
        assert!(
            reasons.iter().any(|reason| reason.contains(SHARED)),
            "{name}: {reasons:?}"
        );
        assert!(host.actions.is_empty(), "{name}");
        assert_eq!(fixture.snapshot(), before, "{name}");
    }
}

#[test]
fn macos_gains_no_shared_parent_allowance() {
    let (fixture, mut host) = setup(Platform::MacOs);
    let gid = shared_gid();
    share(&fixture, "private/var", gid, 0o775);
    let group = format!("root:x:0:\nsyslog:x:{gid}:\n");
    fixture.put("private/etc/group", group.as_bytes(), 0o644);
    let reasons = plan(&fixture, &mut host).expect_err("macos");
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("private/var is group or world writable")),
        "{reasons:?}"
    );
}

#[test]
fn as_root_only_a_root_owned_parent_and_group_file_earn_the_allowance() {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to give test-owned files other owners");
        return;
    }
    let cases: [(&str, Change, Option<&str>); 3] = [
        ("root owns both", |_, _| {}, None),
        (
            "nonroot parent",
            |f, g| chown(f.path("var/log"), Some(1), Some(g)).unwrap(),
            Some("var/log is owned by uid 1"),
        ),
        (
            "nonroot group file",
            |f, _| chown(f.path("etc/group"), Some(1), None).unwrap(),
            Some(SHARED),
        ),
    ];
    for (name, change, refusal) in cases {
        let (fixture, mut host, gid) = shared_log(SYSLOG);
        change(&fixture, gid);
        match (plan(&fixture, &mut host), refusal) {
            (Ok(steps), None) => assert!(
                steps.contains(&Step::CreateDir(p("/var/log/hush"))),
                "{name}: {steps:?}"
            ),
            (Err(reasons), Some(expected)) => assert!(
                reasons.iter().any(|reason| reason.contains(expected)),
                "{name}: {reasons:?}"
            ),
            (outcome, _) => panic!("{name}: {outcome:?}"),
        }
    }
}
