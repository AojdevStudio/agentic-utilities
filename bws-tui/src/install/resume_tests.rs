//! Partial installs: injected native failures, changes during apply and
//! leftover staging links all stop safely and resume to the clean result.

use super::apply::apply;
use super::apply_harness::{install, Token};
use super::custody::STAGE_PREFIX;
use super::fake_host_harness::FakeHost;
use super::fixture_harness::Fixture;
use super::host::Action;
use super::layout::Platform;
use super::observe::observe;
use super::plan::{Plan, Step};
use std::fs;
use std::path::PathBuf;

fn fresh() -> (Fixture, FakeHost) {
    let fixture = Fixture::new(Platform::Linux);
    let host = FakeHost::new(fixture.temp.path(), Platform::Linux);
    (fixture, host)
}

#[test]
fn a_native_failure_before_each_operation_resumes_to_the_clean_result() {
    let (clean, mut clean_host) = fresh();
    install(&clean, &mut clean_host, &mut Token::default()).unwrap();
    let expected = clean.snapshot();
    for failing in 0..clean_host.actions.len() {
        let (fixture, mut host) = fresh();
        host.fail_at = Some(failing);
        let mut token = Token::default();
        let failure = install(&fixture, &mut host, &mut token).unwrap_err();
        assert_eq!(
            host.actions.len(),
            failing + 1,
            "operations ran after the failure at {failing}"
        );
        assert!(!failure.done.is_empty(), "{failing}");
        host.fail_at = None;
        install(&fixture, &mut host, &mut token).unwrap();
        let stored_before_failure = failure
            .done
            .iter()
            .any(|step| matches!(step, Step::StoreCredential(_)));
        assert_eq!(
            token.reads,
            if stored_before_failure { 1 } else { 2 },
            "{failing}"
        );
        assert_eq!(
            fixture.snapshot(),
            expected,
            "resume after failure {failing} differs from a clean install"
        );
    }
}

#[test]
fn a_change_during_apply_stops_before_any_later_step() {
    let (fixture, mut host) = fresh();
    let unit = fixture.path("etc/systemd/system/hush.service");
    let foreign = unit.clone();
    host.on_act = Some(Box::new(move |action| {
        if matches!(action, Action::CreateHostKey) {
            fs::write(&foreign, b"operator unit\n").unwrap();
        }
    }));
    let failure = install(&fixture, &mut host, &mut Token::default()).unwrap_err();
    assert_eq!(
        failure.step,
        format!(
            "{}",
            Step::Publish(PathBuf::from("/etc/systemd/system/hush.service"))
        )
    );
    assert_eq!(fs::read(&unit).unwrap(), b"operator unit\n");
    assert!(!host.actions.iter().any(|action| matches!(
        action,
        Action::DaemonReload | Action::EnableUnit | Action::StartUnit
    )));
}

#[test]
fn a_change_after_planning_refuses_before_input_or_writes() {
    let (fixture, mut host) = fresh();
    let plan = Plan::from_observation(observe(fixture.layout(), &mut host)).unwrap();
    fixture.mkdir_mode("usr/local/bin", 0o775);
    let before = fixture.snapshot();
    let mut token = Token::default();
    let failure = apply(plan, &mut host, &mut token).unwrap_err();
    assert!(failure.done.is_empty());
    assert_eq!((token.reads, host.actions.len()), (0, 0));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_leftover_own_stage_link_is_finished_without_input() {
    let (fixture, mut host) = fresh();
    fixture.mkdir("etc/hush", 0o700);
    let stage = format!("etc/hush/{STAGE_PREFIX}token.cred.77.5");
    fixture.put(&stage, b"complete ciphertext", 0o600);
    fs::hard_link(fixture.path(&stage), fixture.path("etc/hush/token.cred")).unwrap();
    let mut token = Token::default();
    let done = install(&fixture, &mut host, &mut token).unwrap();
    assert!(
        done.iter()
            .any(|step| matches!(step, Step::FinishStage(..))),
        "{done:?}"
    );
    assert_eq!(token.reads, 0);
    assert!(!fixture.path(&stage).exists());
    assert_eq!(
        fs::read(fixture.path("etc/hush/token.cred")).unwrap(),
        b"complete ciphertext"
    );
}
