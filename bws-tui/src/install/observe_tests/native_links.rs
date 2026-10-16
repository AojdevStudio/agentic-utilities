//! Protected native links: authselect's system-auth and a merged usr/sbin.
//! Each target must stay inside the root on a checked chain.

use super::{p, plan, setup};
use crate::install::layout::Platform;
use crate::install::observe::Creation;
use crate::install::plan::Step;
use std::os::unix::fs::symlink;

type Change = fn(&super::Fixture);

/// An authselect host: no Debian files, system-auth links into etc/authselect.
fn authselect(fixture: &super::Fixture, link: &str) {
    for name in ["common-auth", "common-account"] {
        std::fs::remove_file(fixture.path(&format!("etc/pam.d/{name}"))).unwrap();
    }
    fixture.mkdir("etc/authselect", 0o755);
    fixture.put(
        "etc/authselect/system-auth",
        b"auth required pam_unix.so\n",
        0o644,
    );
    symlink(link, fixture.path("etc/pam.d/system-auth")).unwrap();
}

/// A merged sbin host: usr/sbin -> bin, with nologin in usr/bin.
fn merged_sbin(fixture: &super::Fixture) {
    std::fs::remove_file(fixture.path("usr/sbin/nologin")).unwrap();
    std::fs::remove_dir(fixture.path("usr/sbin")).unwrap();
    fixture.mkdir("usr/bin", 0o755);
    fixture.put("usr/bin/nologin", b"fixture\n", 0o755);
    symlink("bin", fixture.path("usr/sbin")).unwrap();
}

#[test]
fn an_authselect_system_auth_link_renders_the_pam_service() {
    let (fixture, mut host) = setup(Platform::Linux);
    authselect(&fixture, "../authselect/system-auth");
    let before = fixture.snapshot();
    let steps = plan(&fixture, &mut host).expect("authselect layout");
    assert!(
        steps.contains(&Step::Publish(p("/etc/pam.d/hush"))),
        "{steps:?}"
    );
    assert!(host.actions.is_empty());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_merged_usr_sbin_keeps_the_recorded_nologin_shell() {
    let (fixture, mut host) = setup(Platform::Linux);
    merged_sbin(&fixture);
    let before = fixture.snapshot();
    let steps = plan(&fixture, &mut host).expect("merged sbin layout");
    assert!(
        steps.contains(&Step::CreateAccount(Creation::Linux {
            gid: 65534,
            shell: "/usr/sbin/nologin".into()
        })),
        "{steps:?}"
    );
    assert!(host.actions.is_empty());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn unsafe_or_escaping_native_links_still_refuse() {
    let cases: [(&str, &str, Change); 8] = [
        ("PAM", "target outside the root", |f| {
            authselect(f, "../../../etc/authselect/system-auth")
        }),
        ("PAM", "absolute target the kernel resolves outside", |f| {
            authselect(f, "/etc/authselect/system-auth")
        }),
        ("PAM", "writable target directory", |f| {
            authselect(f, "../authselect/system-auth");
            f.mkdir_mode("etc/authselect", 0o777);
        }),
        ("PAM", "group writable target", |f| {
            authselect(f, "../authselect/system-auth");
            f.mkdir_mode("etc/authselect/system-auth", 0o664);
        }),
        ("PAM", "link loop", |f| authselect(f, "system-auth")),
        ("nonlogin shell", "writable link target directory", |f| {
            merged_sbin(f);
            f.mkdir_mode("usr/bin", 0o777);
        }),
        ("nonlogin shell", "group writable shell", |f| {
            merged_sbin(f);
            f.mkdir_mode("usr/bin/nologin", 0o775);
        }),
        ("symbolic link", "a linked hush PAM artifact", |f| {
            f.mkdir("etc/authselect", 0o755);
            f.put("etc/authselect/hush", b"auth required pam_unix.so\n", 0o644);
            symlink("../authselect/hush", f.path("etc/pam.d/hush")).unwrap();
        }),
    ];
    for (expected, name, change) in cases {
        let (fixture, mut host) = setup(Platform::Linux);
        change(&fixture);
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
