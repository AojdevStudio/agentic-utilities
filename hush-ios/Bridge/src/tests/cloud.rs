//! A local mock Bitwarden cloud for SDK tests.
//!
//! Token, payload, and cipher strings are the public fixtures from the
//! `bitwarden-sm` 3.0.0 test suite. Every cipher string opens to "TEST" with
//! the organization key inside the payload. The JWT is built at run time.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use tokio::runtime::{Builder, Runtime};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::call;
use crate::ffi::{HushSession, hush_session_drop};
use crate::session::settings;

pub const ORG: &str = "f4e44a7f-1190-432a-9d4a-af96013127cb";
pub const SECRET: &str = "15744a66-341a-4c62-af50-af960166b6bc";
pub const PROJECT: &str = "8d5a6b1e-2f3c-4a7d-9e0b-1c2d3e4f5a6b";
pub const TOKEN: &str = "0.ec2c1d46-6a4b-4751-a310-af9601317f2d.C2IgxjjLF7qSshsbwe8JGcbM075YXw:X8vbvA0bduihIDe/qrzIQQ==";
const PAYLOAD: &str = "2.E9fE8+M/VWMfhhim1KlCbQ==|eLsHR484S/tJbIkM6spnG/HP65tj9A6Tba7kAAvUp+rYuQmGLixiOCfMsqt5OvBctDfvvr/AesBu7cZimPLyOEhqEAjn52jF0eaI38XZfeOG2VJl0LOf60Wkfh3ryAMvfvLj3G4ZCNYU8sNgoC2+IQ==|lNApuCQ4Pyakfo/wwuuajWNaEX/2MW8/3rjXB/V7n+k=";
pub const ENC_KEY: &str = "2.pMS6/icTQABtulw52pq2lg==|XXbxKxDTh+mWiN1HjH2N1w==|Q6PkuT+KX/axrgN9ubD5Ajk2YNwxQkgs3WJM0S0wtG8=";
const ENC_VALUE: &str = "2.Gl34n9JYABC7V21qHcBzHg==|c1Ds244pob7i+8+MXe4++w==|Shimz/qKMYZmzSFWdeBzFb9dFz7oF6Uv9oqkws7rEe0=";
const ENC_NOTE: &str = "2.Cn9ABJy7+WfR4uUHwdYepg==|+nbJyU/6hSknoa5dcEJEUg==|1DTp/ZbwGO3L3RN+VMsCHz8XDr8egn/M5iSitGGysPA=";
pub const DATE: &str = "2023-01-26T21:46:02.2182556Z";

/// A mock Bitwarden cloud plus one bridge session pointed at it.
pub struct Cloud {
    runtime: Runtime,
    server: MockServer,
    pub session: *mut HushSession,
}

impl Cloud {
    pub fn start(deadline: Duration) -> Self {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let server = runtime.block_on(MockServer::start());
        let uri = server.uri();
        let settings = settings(&format!("{uri}/identity"), &format!("{uri}/api"));
        let session = HushSession::open(settings, deadline).expect("session");
        let cloud = Self {
            runtime,
            server,
            session: Box::into_raw(Box::new(session)),
        };
        cloud.mount(
            "POST",
            "/identity/connect/token",
            ResponseTemplate::new(200).set_body_json(identity_body()),
        );
        cloud
    }

    pub fn mount(&self, verb: &str, route: &str, response: ResponseTemplate) {
        let mock = Mock::given(method(verb))
            .and(path(route))
            .respond_with(response);
        self.runtime.block_on(mock.mount(&self.server));
    }

    pub fn reset(&self) {
        self.runtime.block_on(self.server.reset());
    }

    pub fn call(&self, request: Value) -> Value {
        call(self.session, request.to_string().as_bytes())
    }

    pub fn login(&self) -> Value {
        self.call(json!({"op": "login", "token": TOKEN}))
    }

    /// Every request the server saw, as "METHOD /path".
    pub fn seen(&self) -> Vec<String> {
        let requests = self
            .runtime
            .block_on(self.server.received_requests())
            .unwrap_or_default();
        requests
            .iter()
            .map(|r| format!("{} {}", r.method, r.url.path()))
            .collect()
    }
}

impl Drop for Cloud {
    fn drop(&mut self) {
        unsafe { hush_session_drop(self.session) };
    }
}

fn identity_body() -> Value {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"at+jwt"}"#);
    let claims = json!({"exp": 4_102_444_800u64, "sub": "service-account", "organization": ORG, "scope": ["api.secrets"]});
    let claims = URL_SAFE_NO_PAD.encode(claims.to_string());
    json!({
        "access_token": format!("{header}.{claims}.unsigned"),
        "expires_in": 3600,
        "token_type": "Bearer",
        "scope": "api.secrets",
        "encrypted_payload": PAYLOAD,
    })
}

pub fn secret_body() -> Value {
    json!({
        "id": SECRET, "organizationId": ORG, "key": ENC_KEY, "value": ENC_VALUE, "note": ENC_NOTE,
        "creationDate": DATE, "revisionDate": DATE,
        "projects": [{"id": PROJECT, "name": ENC_KEY}], "object": "secret",
    })
}

pub fn create_request() -> Value {
    json!({"op": "create", "key": "TEST", "value": "TEST", "note": "TEST", "projectID": PROJECT})
}

pub fn secrets_route() -> String {
    format!("/api/organizations/{ORG}/secrets")
}
