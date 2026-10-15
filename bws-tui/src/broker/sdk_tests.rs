//! Real SDK writes against a local mock Bitwarden cloud. The token, payload,
//! and cipher strings are the public fixtures from the `bitwarden-sm` 3.0.0
//! test suite, the same ones the iOS bridge tests use. Every cipher string
//! opens to "TEST" with the organization key inside the payload.

use super::material::SecretMaterial;
use super::meta::{NewSecret, SecretUpdate, WriteFailure};
use super::request::{RequestControl, WriteAttempt};
use super::sdk::SdkWriter;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bitwarden::secrets_manager::secrets::SecretGetRequest;
use bitwarden::secrets_manager::{
    AccessTokenLoginRequest, ClientSettings, DeviceType, SecretsManagerClient,
};
use secrecy::SecretString;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(super) const ORG: &str = "f4e44a7f-1190-432a-9d4a-af96013127cb";
const SECRET: &str = "15744a66-341a-4c62-af50-af960166b6bc";
pub(super) const PROJECT: &str = "8d5a6b1e-2f3c-4a7d-9e0b-1c2d3e4f5a6b";
const TOKEN: &str = "0.ec2c1d46-6a4b-4751-a310-af9601317f2d.C2IgxjjLF7qSshsbwe8JGcbM075YXw:X8vbvA0bduihIDe/qrzIQQ==";
const PAYLOAD: &str = "2.E9fE8+M/VWMfhhim1KlCbQ==|eLsHR484S/tJbIkM6spnG/HP65tj9A6Tba7kAAvUp+rYuQmGLixiOCfMsqt5OvBctDfvvr/AesBu7cZimPLyOEhqEAjn52jF0eaI38XZfeOG2VJl0LOf60Wkfh3ryAMvfvLj3G4ZCNYU8sNgoC2+IQ==|lNApuCQ4Pyakfo/wwuuajWNaEX/2MW8/3rjXB/V7n+k=";
const ENC: &str = "2.pMS6/icTQABtulw52pq2lg==|XXbxKxDTh+mWiN1HjH2N1w==|Q6PkuT+KX/axrgN9ubD5Ajk2YNwxQkgs3WJM0S0wtG8=";
const DATE: &str = "2023-01-26T21:46:02.2182556Z";
const VALUE: &str = "FAKE-sdk-value-0009";

/// A mock cloud with a working login. The runtime only starts and inspects it.
pub(super) struct Cloud {
    runtime: tokio::runtime::Runtime,
    server: MockServer,
}

impl Cloud {
    pub(super) fn start() -> Self {
        Self::with_login_delay(Duration::ZERO)
    }

    /// A cloud whose login answers only after `delay`.
    pub(super) fn with_login_delay(delay: Duration) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let server = runtime.block_on(MockServer::start());
        let cloud = Self { runtime, server };
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"at+jwt"}"#);
        let claims = json!({"exp": 4_102_444_800u64, "sub": "service-account", "organization": ORG, "scope": ["api.secrets"]});
        let login = json!({
            "access_token": format!("{header}.{}.unsigned", URL_SAFE_NO_PAD.encode(claims.to_string())),
            "expires_in": 3600, "token_type": "Bearer", "scope": "api.secrets",
            "encrypted_payload": PAYLOAD,
        });
        cloud.mount_delayed("POST", "/identity/connect/token", 200, login, delay);
        cloud
    }

    fn mount(&self, verb: &str, route: &str, status: u16, body: Value) {
        self.mount_delayed(verb, route, status, body, Duration::ZERO);
    }

    pub(super) fn mount_delayed(
        &self,
        verb: &str,
        route: &str,
        status: u16,
        body: Value,
        delay: Duration,
    ) {
        let reply = ResponseTemplate::new(status).set_body_json(body);
        let mock = Mock::given(method(verb))
            .and(path(route))
            .respond_with(reply.set_delay(delay));
        self.runtime.block_on(mock.mount(&self.server));
    }

    /// How many login requests reached the cloud.
    pub(super) fn logins(&self) -> usize {
        let requests = self.runtime.block_on(self.server.received_requests());
        let requests = requests.unwrap_or_default();
        requests
            .iter()
            .filter(|r| r.url.path().starts_with("/identity"))
            .count()
    }

    pub(super) fn writer(&self) -> SdkWriter {
        let uri = self.server.uri();
        let token = SecretString::from(TOKEN.to_string());
        SdkWriter::new(token, &format!("{uri}/identity"), &format!("{uri}/api"))
    }

    /// Every request after login, as "METHOD /path" plus its body.
    pub(super) fn writes(&self) -> Vec<(String, String)> {
        let requests = self.runtime.block_on(self.server.received_requests());
        requests
            .unwrap_or_default()
            .iter()
            .filter(|r| !r.url.path().starts_with("/identity"))
            .map(|r| {
                let body = String::from_utf8_lossy(&r.body).into_owned();
                (format!("{} {}", r.method, r.url.path()), body)
            })
            .collect()
    }
}

/// Open what one write sent. A second SDK client reads the captured
/// ciphertexts back through a mock `GET` and decrypts key, value, and note.
fn sent_plaintext(cloud: &Cloud, body: &str) -> (String, String, String) {
    let sent: Value = serde_json::from_str(body).unwrap();
    let mut echoed = secret_body();
    for field in ["key", "value", "note"] {
        echoed[field] = sent[field].clone();
    }
    cloud.mount("GET", &format!("/api/secrets/{SECRET}"), 200, echoed);
    let uri = cloud.server.uri();
    let client = {
        let _context = cloud.runtime.enter();
        SecretsManagerClient::new(Some(ClientSettings {
            identity_url: format!("{uri}/identity"),
            api_url: format!("{uri}/api"),
            user_agent: "hush test".into(),
            device_type: DeviceType::SDK,
            device_identifier: None,
            bitwarden_client_version: None,
            bitwarden_package_type: None,
        }))
    };
    let login = AccessTokenLoginRequest {
        access_token: TOKEN.into(),
        state_file: None,
    };
    cloud
        .runtime
        .block_on(client.auth().login_access_token(&login))
        .unwrap();
    let get = SecretGetRequest {
        id: Uuid::parse_str(SECRET).unwrap(),
    };
    let secret = cloud.runtime.block_on(client.secrets().get(&get)).unwrap();
    (secret.key, secret.value, secret.note)
}

pub(super) fn secret_body() -> Value {
    json!({
        "id": SECRET, "organizationId": ORG, "key": ENC, "value": ENC, "note": ENC,
        "creationDate": DATE, "revisionDate": DATE,
        "projects": [{"id": PROJECT, "name": ENC}], "object": "secret",
    })
}

/// A write outside any human request: no client to watch, no request
/// deadline. Each control is leaked, which a test process can afford.
pub(super) fn detached() -> WriteAttempt<'static> {
    WriteAttempt::new(Box::leak(Box::new(RequestControl::detached())))
}

fn value() -> SecretMaterial {
    SecretMaterial::new(VALUE.as_bytes().to_vec())
}

fn new_secret() -> NewSecret {
    NewSecret {
        project_id: PROJECT.into(),
        key: "FRESH_KEY".into(),
        note: "fresh note".into(),
    }
}

#[test]
fn create_sends_one_encrypted_request() {
    let cloud = Cloud::start();
    let route = format!("/api/organizations/{ORG}/secrets");
    cloud.mount("POST", &route, 200, secret_body());
    let meta = cloud
        .writer()
        .create(&new_secret(), &value(), &mut detached())
        .unwrap();
    assert_eq!((meta.id.as_str(), meta.key.as_str()), (SECRET, "TEST"));
    assert_eq!(meta.project_id.as_deref(), Some(PROJECT));
    let writes = cloud.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].0, format!("POST {route}"));
    assert!(
        !writes[0].1.contains(VALUE),
        "the value left the daemon unencrypted"
    );
    let sent = sent_plaintext(&cloud, &writes[0].1);
    assert_eq!(
        sent,
        ("FRESH_KEY".into(), VALUE.into(), "fresh note".into())
    );
}

#[test]
fn update_writes_the_full_record_once() {
    let cloud = Cloud::start();
    let route = format!("/api/secrets/{SECRET}");
    cloud.mount("PUT", &route, 200, secret_body());
    let update = SecretUpdate {
        id: SECRET.into(),
        project_id: Some(PROJECT.into()),
        key: "TEST".into(),
        note: "rotated".into(),
    };
    let meta = cloud
        .writer()
        .update(&update, &value(), &mut detached())
        .unwrap();
    assert_eq!(meta.id, SECRET);
    let writes = cloud.writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].0, format!("PUT {route}"));
    assert!(!writes[0].1.contains(VALUE));
    let sent = sent_plaintext(&cloud, &writes[0].1);
    assert_eq!(sent, ("TEST".into(), VALUE.into(), "rotated".into()));
}

#[test]
fn failures_say_whether_anything_may_have_been_written() {
    let cloud = Cloud::start();
    let route = format!("/api/organizations/{ORG}/secrets");
    cloud.mount("POST", &route, 400, json!({"message": "rejected"}));
    assert_eq!(
        cloud
            .writer()
            .create(&new_secret(), &value(), &mut detached())
            .unwrap_err(),
        WriteFailure::Rejected
    );

    let cloud = Cloud::start();
    cloud.mount("POST", &route, 500, json!({"message": "server error"}));
    assert_eq!(
        cloud
            .writer()
            .create(&new_secret(), &value(), &mut detached())
            .unwrap_err(),
        WriteFailure::Uncertain
    );

    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let base = format!("http://{closed}");
    let token = SecretString::from(TOKEN.to_string());
    let unreachable = SdkWriter::new(token, &format!("{base}/identity"), &format!("{base}/api"));
    let failure = unreachable
        .create(&new_secret(), &value(), &mut detached())
        .unwrap_err();
    assert_eq!(
        failure,
        WriteFailure::Rejected,
        "a failed login wrote nothing"
    );
}
