//! Real SDK calls through the C ABI against a local mock server.

use std::net::TcpListener;
use std::time::Duration;

use serde_json::json;
use wiremock::ResponseTemplate;

use super::cloud::{
    Cloud, DATE, ENC_KEY, ORG, PROJECT, SECRET, TOKEN, create_request, secret_body, secrets_route,
};
use super::{call, code};
use crate::ffi::{HushSession, hush_session_drop};
use crate::session::settings;

#[test]
fn login_returns_the_token_organization() {
    let cloud = Cloud::start(Duration::from_secs(5));
    let envelope = cloud.login();
    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["data"]["organizationID"], ORG);
    // A second login on an unlocked session is a caller mistake.
    assert_eq!(code(&cloud.login()), Some("invalid_request"));
}

#[test]
fn rejected_or_unreachable_login_maps_to_stable_codes() {
    let cloud = Cloud::start(Duration::from_secs(5));
    cloud.reset();
    let fail = json!({"error": "invalid_client", "error_description": "invalid_client", "ErrorModel": {"Message": "", "Object": "error"}});
    for (status, body, expected) in [
        (400, fail, "authentication"),
        (400, json!({"error": "invalid_client"}), "authentication"),
        (503, json!({}), "network"),
    ] {
        cloud.reset();
        cloud.mount(
            "POST",
            "/identity/connect/token",
            ResponseTemplate::new(status).set_body_json(body),
        );
        assert_eq!(code(&cloud.login()), Some(expected), "status {status}");
    }

    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("port")
        .port();
    let closed = format!("http://127.0.0.1:{port}");
    let settings = settings(&format!("{closed}/identity"), &format!("{closed}/api"));
    let session = Box::into_raw(Box::new(
        HushSession::open(settings, Duration::from_secs(5)).expect("session"),
    ));
    let request = json!({"op": "login", "token": TOKEN}).to_string();
    assert_eq!(code(&call(session, request.as_bytes())), Some("network"));
    unsafe { hush_session_drop(session) };
}

#[test]
fn catalog_returns_metadata_and_never_fetches_values() {
    let cloud = Cloud::start(Duration::from_secs(5));
    cloud.login();
    let project = json!({"id": PROJECT, "organizationId": ORG, "name": ENC_KEY, "creationDate": DATE, "revisionDate": DATE, "object": "project"});
    cloud.mount(
        "GET",
        &format!("/api/organizations/{ORG}/projects"),
        ResponseTemplate::new(200).set_body_json(json!({"data": [project], "object": "list"})),
    );
    let summary = json!({"id": SECRET, "organizationId": ORG, "key": ENC_KEY, "creationDate": DATE, "revisionDate": DATE, "projects": [{"id": PROJECT, "name": ENC_KEY}]});
    cloud.mount(
        "GET",
        &secrets_route(),
        ResponseTemplate::new(200).set_body_json(
            json!({"secrets": [summary], "projects": [], "object": "SecretsWithProjectsList"}),
        ),
    );

    let envelope = cloud.call(json!({"op": "catalog"}));
    assert_eq!(
        envelope["data"],
        json!({"projects": [{"id": PROJECT, "name": "TEST"}], "secrets": [{"id": SECRET, "key": "TEST", "projectIDs": [PROJECT]}]}),
        "{envelope}"
    );
    assert!(
        cloud.seen().iter().all(|r| !r.contains("/api/secrets")),
        "{:?}",
        cloud.seen()
    );
}

#[test]
fn read_returns_the_decrypted_secret() {
    let cloud = Cloud::start(Duration::from_secs(5));
    cloud.login();
    cloud.mount(
        "GET",
        &format!("/api/secrets/{SECRET}"),
        ResponseTemplate::new(200).set_body_json(secret_body()),
    );
    let envelope = cloud.call(json!({"op": "read", "id": SECRET}));
    assert_eq!(
        envelope["data"],
        json!({"id": SECRET, "key": "TEST", "value": "TEST", "note": "TEST", "projectID": PROJECT}),
        "{envelope}"
    );
}

#[test]
fn read_failures_map_to_stable_codes() {
    let cloud = Cloud::start(Duration::from_millis(500));
    cloud.login();
    let route = format!("/api/secrets/{SECRET}");
    for (response, expected) in [
        (ResponseTemplate::new(401), "authentication"),
        (ResponseTemplate::new(403), "permission"),
        (ResponseTemplate::new(404), "permission"),
        (ResponseTemplate::new(500), "network"),
        (
            ResponseTemplate::new(200)
                .set_body_json(secret_body())
                .set_delay(Duration::from_secs(2)),
            "network",
        ),
        (
            ResponseTemplate::new(200).set_body_json(json!({"id": SECRET})),
            "internal",
        ),
    ] {
        cloud.reset();
        cloud.mount("GET", &route, response);
        let envelope = cloud.call(json!({"op": "read", "id": SECRET}));
        assert_eq!(code(&envelope), Some(expected), "{envelope}");
    }
}

#[test]
fn create_returns_the_round_trip_without_a_second_read() {
    let cloud = Cloud::start(Duration::from_secs(5));
    cloud.login();
    cloud.mount(
        "POST",
        &secrets_route(),
        ResponseTemplate::new(200).set_body_json(secret_body()),
    );
    let envelope = cloud.call(create_request());
    assert_eq!(envelope["data"]["value"], "TEST", "{envelope}");
    assert_eq!(envelope["data"]["projectID"], PROJECT);
    let api: Vec<String> = cloud
        .seen()
        .into_iter()
        .filter(|r| r.contains("/api/"))
        .collect();
    assert_eq!(api, vec![format!("POST {}", secrets_route())]);
}

#[test]
fn uncertain_create_is_unknown_and_never_retried() {
    let cloud = Cloud::start(Duration::from_millis(500));
    cloud.login();
    for response in [
        ResponseTemplate::new(500),
        ResponseTemplate::new(502),
        ResponseTemplate::new(200)
            .set_body_json(secret_body())
            .set_delay(Duration::from_secs(2)),
        ResponseTemplate::new(200).set_body_json(json!({"id": SECRET})),
    ] {
        cloud.reset();
        cloud.mount("POST", &secrets_route(), response);
        assert_eq!(code(&cloud.call(create_request())), Some("unknown_create"));
        let posts = cloud
            .seen()
            .iter()
            .filter(|r| r.starts_with("POST /api/"))
            .count();
        assert_eq!(posts, 1);
    }
}

#[test]
fn definite_create_rejections_stay_distinct() {
    let cloud = Cloud::start(Duration::from_secs(5));
    cloud.login();
    for (status, expected) in [
        (400, "invalid_request"),
        (401, "authentication"),
        (403, "permission"),
        (429, "network"),
    ] {
        cloud.reset();
        cloud.mount("POST", &secrets_route(), ResponseTemplate::new(status));
        assert_eq!(
            code(&cloud.call(create_request())),
            Some(expected),
            "status {status}"
        );
    }
}
