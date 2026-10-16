//! Observation, planning and status on real trees with a fake native boundary.

use super::facts::AccountRecord;
use super::fake_host_harness::FakeHost;
use super::fixture_harness::Fixture;
use super::layout::Platform;
use super::observe::{observe, Creation};
use super::plan::{Plan, Step};
use super::status::Status;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

type Mutation = fn(&Fixture, &mut FakeHost);

fn setup(platform: Platform) -> (Fixture, FakeHost) {
    let fixture = Fixture::new(platform);
    let host = FakeHost::new(fixture.temp.path(), platform);
    (fixture, host)
}

fn plan(fixture: &Fixture, host: &mut FakeHost) -> Result<Vec<Step>, Vec<String>> {
    Plan::from_observation(observe(fixture.layout(), host))
        .map(|plan| plan.steps().to_vec())
        .map_err(|refusal| refusal.0)
}

fn status(fixture: &Fixture, host: &mut FakeHost) -> Vec<String> {
    Status(&observe(fixture.layout(), host))
        .to_string()
        .lines()
        .map(String::from)
        .collect()
}

fn p(path: &str) -> PathBuf {
    PathBuf::from(path)
}

#[test]
fn a_fresh_linux_host_plans_every_step_in_order_without_writing() {
    let (fixture, mut host) = setup(Platform::Linux);
    let before = fixture.snapshot();
    let observed = observe(fixture.layout(), &mut host);
    let plan = Plan::from_observation(observed).unwrap();
    assert_eq!(
        plan.steps(),
        [
            Step::CreateDir(p("/etc/hush")),
            Step::CreateDir(p("/var/log/hush")),
            Step::CreateAccount(Creation::Linux {
                gid: 65534,
                shell: "/usr/sbin/nologin".into()
            }),
            Step::CreateHostKey,
            Step::StoreCredential(p("/etc/hush/token.cred")),
            Step::Publish(p("/etc/pam.d/hush")),
            Step::Publish(p("/etc/systemd/system/hush.service")),
            Step::Reload,
            Step::Enable,
            Step::Start,
        ]
    );
    assert!(plan.needs_token());
    assert!(host.actions.is_empty());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_fresh_macos_host_plans_one_complete_account_and_a_launchd_load() {
    let (fixture, mut host) = setup(Platform::MacOs);
    assert_eq!(
        plan(&fixture, &mut host).unwrap(),
        [
            Step::CreateDir(p("/private/var/db/hush")),
            Step::CreateAccount(Creation::Mac {
                uid: 498,
                nobody_gid: -2
            }),
            Step::StoreCredential(p("/private/var/db/hush/token")),
            Step::Publish(p("/Library/LaunchDaemons/hush.plist")),
            Step::Load,
        ]
    );
}

#[test]
fn known_conflicts_refuse_before_any_input_or_write() {
    let cases: [(&str, Mutation); 8] = [
        ("group or world writable", |f, _| {
            f.mkdir_mode("usr/local/bin", 0o775)
        }),
        ("has mode 644", |f, _| {
            f.mkdir("etc/hush", 0o700);
            f.put("etc/hush/token.cred", b"x", 0o644);
        }),
        ("247 or newer", |_, h| h.systemd = 246),
        ("hushrun", |_, h| {
            h.account = Some(AccountRecord {
                uid: 998,
                gid: 65534,
                home: "/nonexistent".into(),
                shell: "/usr/sbin/nologin".into(),
                locked: false,
            })
        }),
        ("hushrun", |_, h| {
            h.account = Some(AccountRecord {
                uid: 998,
                gid: 100,
                home: "/nonexistent".into(),
                shell: "/usr/sbin/nologin".into(),
                locked: true,
            })
        }),
        ("PAM", |f, _| {
            std::fs::remove_file(f.path("etc/pam.d/common-auth")).unwrap()
        }),
        ("differs", |f, _| {
            f.put("etc/systemd/system/hush.service", b"edited\n", 0o644)
        }),
        ("not the managed hush daemon", |f, _| {
            f.mkdir("run/hush", 0o755);
            std::mem::forget(UnixListener::bind(f.path("run/hush/hush.sock")).unwrap());
        }),
    ];
    for (expected, mutate) in cases {
        let (fixture, mut host) = setup(Platform::Linux);
        mutate(&fixture, &mut host);
        let before = fixture.snapshot();
        let reasons = plan(&fixture, &mut host).expect_err(expected);
        assert!(
            reasons.iter().any(|reason| reason.contains(expected)),
            "{expected}: {reasons:?}"
        );
        assert!(host.actions.is_empty(), "{expected}");
        assert_eq!(fixture.snapshot(), before, "{expected}");
    }
}

#[test]
fn an_operator_pam_service_is_kept_instead_of_rendered() {
    for existing in ["etc/pam.d/hush", "usr/lib/pam.d/hush"] {
        let (fixture, mut host) = setup(Platform::Linux);
        fixture.mkdir("usr/lib", 0o755);
        fixture.mkdir("usr/lib/pam.d", 0o755);
        fixture.put(
            existing,
            b"auth required pam_unix.so\naccount required pam_unix.so\n",
            0o600,
        );
        let steps = plan(&fixture, &mut host).unwrap();
        assert!(
            !steps.contains(&Step::Publish(p("/etc/pam.d/hush"))),
            "{existing}: {steps:?}"
        );
    }
}

#[test]
fn unknown_facts_block_install_and_status_reports_them_without_a_token() {
    let (fixture, mut host) = setup(Platform::Linux);
    host.account = Some(AccountRecord {
        uid: 998,
        gid: 65534,
        home: "/nonexistent".into(),
        shell: "/usr/sbin/nologin".into(),
        locked: true,
    });
    host.shadow_readable = false;
    let reasons = plan(&fixture, &mut host).unwrap_err();
    assert!(
        reasons.iter().any(|reason| reason.contains("needs root")),
        "{reasons:?}"
    );
    let lines = status(&fixture, &mut host);
    assert!(lines[0].starts_with("installed: unknown"), "{lines:?}");
}

#[test]
fn status_reports_six_independent_facts_for_a_fresh_host() {
    let (fixture, mut host) = setup(Platform::Linux);
    let lines = status(&fixture, &mut host);
    let heads: Vec<&str> = lines
        .iter()
        .map(|line| line.split(':').next().unwrap())
        .collect();
    assert_eq!(
        heads,
        [
            "installed",
            "enabled",
            "active",
            "reachable",
            "credential",
            "authentication"
        ]
    );
    assert!(lines[0].starts_with("installed: no"), "{lines:?}");
    assert_eq!(&lines[1..4], ["enabled: no", "active: no", "reachable: no"]);
    assert!(lines[4].starts_with("credential: absent"), "{lines:?}");
    assert!(
        lines[5].starts_with("authentication: not checked"),
        "{lines:?}"
    );
    assert!(host.actions.is_empty() && host.encrypted == 0);
}
