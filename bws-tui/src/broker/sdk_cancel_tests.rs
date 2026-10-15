//! SDK writes on the human path, against a local mock cloud. A cancelled
//! request sends no write. A write that may have reached the cloud is
//! uncertain. Fields the SDK would refuse never reach a login.

use super::backend::VaultBackend;
use super::cancel_harness::Pending;
use super::daemon_harness::FakeAuth;
use super::fdpass_tests::pipe;
use super::human::HumanConfig;
use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use super::operation::{Operation, Refusal, ValueSource};
use super::protocol::ResponseFrame;
use super::request::{RequestControl, WriteAttempt};
use super::sdk::SdkWriter;
use super::sdk_tests::{detached, secret_body, Cloud, ORG, PROJECT};
use super::FakeBackend;
use std::collections::HashMap;
use std::io::Write;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BOUND: Duration = Duration::from_secs(2);

/// Reads from a fake vault in the mock cloud's project, writes through the real SDK.
struct SdkVault {
    reads: FakeBackend,
    writer: SdkWriter,
}

impl VaultBackend for SdkVault {
    fn catalog(&self, control: &RequestControl<'_>) -> anyhow::Result<Vec<SecretMeta>> {
        self.reads.catalog(control)
    }

    fn fetch(
        &self,
        secret: &SecretRef,
        control: &RequestControl<'_>,
    ) -> anyhow::Result<SecretMaterial> {
        self.reads.fetch(secret, control)
    }

    fn projects(&self, control: &RequestControl<'_>) -> anyhow::Result<Vec<ProjectMeta>> {
        self.reads.projects(control)
    }

    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.create(secret, value, attempt)
    }

    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.update(secret, value, attempt)
    }

    fn delete(&self, id: &str, attempt: &mut WriteAttempt<'_>) -> Result<(), WriteFailure> {
        self.reads.delete(id, attempt)
    }
}

fn sdk_vault(cloud: &Cloud) -> Arc<SdkVault> {
    let meta = SecretMeta {
        id: "s1".into(),
        key: "TOKEN".into(),
        project: Some("Test".into()),
        project_id: Some(PROJECT.into()),
        note: String::new(),
    };
    let entry = ("s1".to_string(), (meta, "FAKE-sdk-stored-0014".to_string()));
    let reads = FakeBackend::new(HashMap::from([entry]));
    Arc::new(SdkVault {
        reads,
        writer: cloud.writer(),
    })
}

/// A ready piped add whose client stays connected until the test hangs up.
fn piped_add(vault: Arc<SdkVault>) -> Pending {
    let (reader, mut writer) = pipe();
    writer.write_all(b"FAKE-sdk-new-0015").unwrap();
    drop(writer);
    let op = Operation::Add {
        project: "Test".into(),
        key: "FRESH".into(),
        note: None,
        value: ValueSource::InputFd,
    };
    let config = Arc::new(HumanConfig::new(FakeAuth::panicking()));
    Pending::new(op, &[reader.as_fd()]).serve(vault, config)
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < until, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_client_that_leaves_during_sdk_login_sends_no_write() {
    let cloud = Cloud::with_login_delay(Duration::from_millis(800));
    let route = format!("/api/organizations/{ORG}/secrets");
    cloud.mount_delayed("POST", &route, 200, secret_body(), Duration::ZERO);
    let mut pending = piped_add(sdk_vault(&cloud));
    wait_until("the login request", || cloud.logins() == 1);
    pending.hang_up();
    let reply = pending.reply_within(BOUND);
    std::thread::sleep(Duration::from_millis(1200));
    let cancelled = ResponseFrame::Refused {
        refusal: Refusal::ApprovalCancelled,
    };
    assert_eq!(reply, Some(cancelled));
    assert_eq!(cloud.writes(), [], "a cancelled add reached the cloud");
}

#[test]
fn a_client_that_leaves_after_the_write_started_gets_an_uncertain_write() {
    let cloud = Cloud::start();
    let route = format!("/api/organizations/{ORG}/secrets");
    cloud.mount_delayed(
        "POST",
        &route,
        200,
        secret_body(),
        Duration::from_millis(800),
    );
    let mut pending = piped_add(sdk_vault(&cloud));
    wait_until("the create request", || cloud.writes().len() == 1);
    pending.hang_up();
    let uncertain = ResponseFrame::Refused {
        refusal: Refusal::WriteUncertain {
            operation: "add".into(),
        },
    };
    assert_eq!(pending.reply_within(BOUND), Some(uncertain));
    assert_eq!(cloud.writes().len(), 1, "a write is never retried");
}

/// The SDK copies a refused field into its validation error and logs it, so
/// nothing it would refuse may reach it. Unicode counts characters, not bytes.
#[test]
fn fields_the_sdk_would_refuse_never_reach_a_login() {
    let cloud = Cloud::start();
    let writer = cloud.writer();
    let new = |key: String, note: &str| NewSecret {
        project_id: PROJECT.into(),
        key,
        note: note.into(),
    };
    let short = || SecretMaterial::new(b"FAKE-sdk-short-0016".to_vec());
    let cases = [
        (new("k".repeat(501), ""), short()),
        (new("FRESH".into(), &"n".repeat(7001)), short()),
        (new("FRESH".into(), " \t "), short()),
        (
            new("FRESH".into(), ""),
            SecretMaterial::new("é".repeat(25_001).into_bytes()),
        ),
        (new("FRESH".into(), ""), SecretMaterial::new(Vec::new())),
    ];
    for (secret, value) in cases {
        let refused = writer.create(&secret, &value, &mut detached());
        assert_eq!(
            refused.unwrap_err(),
            WriteFailure::Rejected,
            "{}",
            secret.key.len()
        );
    }
    let update = SecretUpdate {
        id: "15744a66-341a-4c62-af50-af960166b6bc".into(),
        project_id: Some(PROJECT.into()),
        key: "TOKEN".into(),
        note: String::new(),
    };
    let kept = SecretMaterial::new("v".repeat(25_001).into_bytes());
    assert_eq!(
        writer.update(&update, &kept, &mut detached()).unwrap_err(),
        WriteFailure::Rejected
    );
    assert_eq!(cloud.logins(), 0, "an invalid write logged in");
    assert_eq!(cloud.writes(), []);
}
