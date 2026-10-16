//! Manager resume and listener identity after an install already exists.

use super::apply::apply;
use super::apply_harness::{install, Token};
use super::facts::Fact;
use super::fake_host_harness::FakeHost;
use super::fixture_harness::Fixture;
use super::host::Action;
use super::layout::Platform;
use super::observe::observe;
use super::plan::Plan;
use super::status::Status;
use std::os::unix::net::UnixListener;

fn installed(platform: Platform) -> (Fixture, FakeHost) {
    let fixture = Fixture::new(platform);
    let mut host = FakeHost::new(fixture.temp.path(), platform);
    install(&fixture, &mut host, &mut Token::default()).unwrap();
    host.actions.clear();
    (fixture, host)
}

#[test]
fn a_stopped_linux_service_resumes_with_start_only() {
    let (fixture, mut host) = installed(Platform::Linux);
    host.stop();
    let _ = std::fs::remove_file(fixture.path("run/hush/hush.sock"));
    let mut token = Token::default();
    install(&fixture, &mut host, &mut token).unwrap();
    assert_eq!(
        (host.actions.as_slice(), token.reads),
        ([Action::StartUnit].as_slice(), 0)
    );
}

#[test]
fn a_disabled_stopped_launchd_job_is_enabled_then_kickstarted() {
    let (fixture, mut host) = installed(Platform::MacOs);
    host.stop();
    host.disabled = Some(true);
    let _ = std::fs::remove_file(fixture.path("private/var/run/hush/hush.sock"));
    let mut token = Token::default();
    install(&fixture, &mut host, &mut token).unwrap();
    assert_eq!(
        host.actions,
        [Action::LaunchdEnable, Action::LaunchdKickstart]
    );
    assert_eq!(token.reads, 0);
}

#[test]
fn status_never_calls_a_foreign_listener_reachable() {
    let fixture = Fixture::new(Platform::Linux);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    fixture.mkdir("run/hush", 0o755);
    let _listener = UnixListener::bind(fixture.path("run/hush/hush.sock")).unwrap();
    let status = Status(&observe(fixture.layout(), &mut host)).to_string();
    assert!(
        status.contains("reachable: no (the listener is not the managed hush daemon)"),
        "{status}"
    );
}

#[test]
fn a_foreign_loaded_command_or_identity_refuses_and_a_missing_one_is_unknown() {
    let (fixture, mut host) = installed(Platform::Linux);
    let plan = Plan::from_observation(observe(fixture.layout(), &mut host)).unwrap();
    assert!(
        plan.steps().is_empty(),
        "the matching loaded service is a no-op"
    );
    let ours = host.loaded_command.borrow().clone().unwrap();
    let mut extra = ours.clone();
    extra.1.push("--extra".into());
    // The same text with different boundaries: `daemon --socket` as one argument.
    let mut merged = ours.clone();
    let socket = merged.1.remove(2);
    merged.1[1] = format!("{} {socket}", merged.1[1]);
    let foreign = (
        "/usr/bin/false".to_string(),
        vec!["/usr/bin/false".to_string()],
    );
    let cases = [
        (Some(foreign), Some("")),
        (Some(extra), Some("")),
        (Some(merged), Some("")),
        (Some(ours.clone()), Some("nobody")),
    ];
    for (command, user) in cases {
        *host.loaded_command.borrow_mut() = command.clone();
        host.user = user.map(String::from);
        let service = observe(fixture.layout(), &mut host).service;
        assert!(
            matches!(service, Fact::Refused(_)),
            "{command:?} {user:?}: {service:?}"
        );
        assert!(Plan::from_observation(observe(fixture.layout(), &mut host)).is_err());
    }
    for (command, user) in [(None, Some("")), (Some(ours), None)] {
        *host.loaded_command.borrow_mut() = command.clone();
        host.user = user.map(String::from);
        let service = observe(fixture.layout(), &mut host).service;
        assert!(
            matches!(service, Fact::Unknown(_)),
            "{command:?} {user:?}: {service:?}"
        );
    }
}

#[test]
fn a_cached_command_the_unit_does_not_render_refuses_and_an_unreadable_list_is_unknown() {
    let (fixture, mut host) = installed(Platform::Linux);
    host.stop();
    let _ = std::fs::remove_file(fixture.path("run/hush/hush.sock"));
    let foreign = || Some(vec!["/usr/bin/false".to_string()]);
    let lists = [
        "ExecCondition",
        "ExecStartPre",
        "ExecStartPost",
        "ExecStop",
        "ExecStopPost",
    ];
    for list in lists {
        for (cached, expected) in [(foreign(), "refused"), (None, "unknown")] {
            host.cached = vec![(list, cached)];
            let got = match observe(fixture.layout(), &mut host).service {
                Fact::Refused(_) => "refused",
                Fact::Unknown(_) => "unknown",
                Fact::Known(_) => "known",
            };
            assert_eq!(got, expected, "{list}");
            assert!(Plan::from_observation(observe(fixture.layout(), &mut host)).is_err());
        }
    }
    host.cached.clear();
    let plan = Plan::from_observation(observe(fixture.layout(), &mut host)).unwrap();
    host.cached = vec![("ExecStartPre", foreign())];
    let failure = apply(plan, &mut host, &mut Token::default()).unwrap_err();
    assert!(host.actions.is_empty(), "{failure:?} {:?}", host.actions);
}

#[test]
fn a_published_unit_discovered_before_reload_is_not_a_foreign_change() {
    let fixture = Fixture::new(Platform::Linux);
    let mut host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    let plan = Plan::from_observation(observe(fixture.layout(), &mut host)).unwrap();
    host.auto_load_unit = true;

    let done = apply(plan, &mut host, &mut Token::default()).unwrap();

    assert_eq!(done.len(), 10);
    assert_eq!(
        host.actions,
        [
            Action::CreateLinuxAccount {
                gid: 65534,
                shell: "/usr/sbin/nologin".into()
            },
            Action::CreateHostKey,
            Action::DaemonReload,
            Action::EnableUnit,
            Action::StartUnit,
        ]
    );
}

#[test]
fn a_foreign_command_loaded_during_apply_stops_before_the_next_manager_action() {
    let (fixture, mut host) = installed(Platform::Linux);
    host.stop();
    host.service.enabled = false;
    let _ = std::fs::remove_file(fixture.path("run/hush/hush.sock"));
    let loaded = host.loaded_command.clone();
    host.on_act = Some(Box::new(move |action| {
        if *action == Action::EnableUnit {
            *loaded.borrow_mut() = Some(("/usr/bin/false".into(), vec!["/usr/bin/false".into()]));
        }
    }));
    let mut token = Token::default();
    let failure = install(&fixture, &mut host, &mut token).unwrap_err();
    assert_eq!(host.actions, [Action::EnableUnit], "{failure:?}");
    assert_eq!(token.reads, 0);
}

#[test]
fn a_macos_policy_parent_opened_during_apply_stops_before_kickstart() {
    let fixture = Fixture::new(Platform::MacOs);
    fixture.mkdir("private/etc/hush", 0o755);
    fixture.put(
        "private/etc/hush/hushd.toml",
        b"require_hushrun = []\n",
        0o600,
    );
    let mut host = FakeHost::new(fixture.temp.path(), Platform::MacOs);
    install(&fixture, &mut host, &mut Token::default()).unwrap();
    host.actions.clear();
    host.stop();
    host.disabled = Some(true);
    let _ = std::fs::remove_file(fixture.path("private/var/run/hush/hush.sock"));
    let parent = fixture.path("private/etc/hush");
    host.on_act = Some(Box::new(move |action| {
        if *action == Action::LaunchdEnable {
            use std::os::unix::fs::PermissionsExt;
            let open = std::fs::Permissions::from_mode(0o777);
            std::fs::set_permissions(&parent, open).unwrap();
        }
    }));
    let failure = install(&fixture, &mut host, &mut Token::default()).unwrap_err();
    assert_eq!(host.actions, [Action::LaunchdEnable], "{failure:?}");
}
