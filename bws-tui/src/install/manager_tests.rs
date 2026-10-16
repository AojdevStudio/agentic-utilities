//! Manager resume and listener identity after an install already exists.

use super::apply_harness::{install, Token};
use super::fake_host_harness::FakeHost;
use super::fixture_harness::Fixture;
use super::host::Action;
use super::layout::Platform;
use super::observe::observe;
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
