//! Boundary behavior that needs no server: pointers, limits, and lock state.

use std::ptr;
use std::time::Duration;

use super::{call, code, take};
use crate::ffi::{
    HushBuffer, HushSession, guarded, hush_buffer_free, hush_session_call, hush_session_drop,
    hush_session_new,
};
use crate::request::MAX_REQUEST_BYTES;
use crate::session::settings_for;

const SECRET_ID: &str = "15744a66-341a-4c62-af50-af960166b6bc";
const PROJECT_ID: &str = "8d5a6b1e-2f3c-4a7d-9e0b-1c2d3e4f5a6b";

fn create(key: &str, value: &str, note: &str) -> Vec<u8> {
    serde_json::json!({"op": "create", "key": key, "value": value, "note": note, "projectID": PROJECT_ID})
        .to_string()
        .into_bytes()
}

#[test]
fn region_selects_a_cloud_or_returns_null() {
    for region in [0, 1] {
        let session = hush_session_new(region);
        assert!(!session.is_null(), "region {region}");
        unsafe { hush_session_drop(session) };
    }
    assert!(hush_session_new(2).is_null());
    assert!(hush_session_new(u32::MAX).is_null());
}

#[test]
fn null_pointers_return_an_envelope_or_do_nothing() {
    let request = br#"{"op":"catalog"}"#;
    let envelope =
        take(unsafe { hush_session_call(ptr::null_mut(), request.as_ptr(), request.len()) });
    assert_eq!(envelope["ok"], false);
    assert_eq!(code(&envelope), Some("invalid_request"));

    let session = hush_session_new(0);
    let envelope = take(unsafe { hush_session_call(session, ptr::null(), 16) });
    assert_eq!(code(&envelope), Some("invalid_request"));

    unsafe {
        hush_session_drop(session);
        hush_session_drop(ptr::null_mut());
        hush_buffer_free(HushBuffer {
            data: ptr::null_mut(),
            len: 0,
        });
    }
}

#[test]
fn malformed_requests_fail_before_any_network_call() {
    let session = hush_session_new(0);
    let long_token = format!(r#"{{"op":"login","token":"{}"}}"#, "a".repeat(1025));
    let cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        vec![0xff, 0xfe, 0xfd],
        b"{not json".to_vec(),
        br#"{"op":"delete","id":"15744a66-341a-4c62-af50-af960166b6bc"}"#.to_vec(),
        br#"{"op":"catalog","extra":true}"#.to_vec(),
        br#"{"op":"read","id":"not-a-uuid"}"#.to_vec(),
        br#"{"op":"read"}"#.to_vec(),
        long_token.into_bytes(),
        create("", "v", ""),
        create("   ", "v", ""),
        create(&"k".repeat(501), "v", ""),
        create("k", "", ""),
        create("k", &"v".repeat(25_001), ""),
        create("k", "v", "\t\n"),
        create("k", "v", &"n".repeat(7_001)),
        vec![b' '; MAX_REQUEST_BYTES + 1],
    ];
    for request in cases {
        let envelope = call(session, &request);
        let shown = String::from_utf8_lossy(&request[..request.len().min(48)]).into_owned();
        assert_eq!(code(&envelope), Some("invalid_request"), "{shown}");
    }
    unsafe { hush_session_drop(session) };
}

#[test]
fn limits_count_characters_not_bytes() {
    let session = hush_session_new(0);
    // 500 four-byte characters is a valid key, so the request reaches the lock check.
    let request = create(&"\u{1F511}".repeat(500), &"\u{1F511}".repeat(25_000), "");
    assert_eq!(code(&call(session, &request)), Some("locked"));
    unsafe { hush_session_drop(session) };
}

#[test]
fn data_operations_are_locked_until_login() {
    let session = hush_session_new(0);
    let read = format!(r#"{{"op":"read","id":"{SECRET_ID}"}}"#).into_bytes();
    for request in [br#"{"op":"catalog"}"#.to_vec(), read, create("k", "v", "")] {
        assert_eq!(code(&call(session, &request)), Some("locked"));
    }
    unsafe { hush_session_drop(session) };
}

#[test]
fn malformed_token_fails_locally() {
    let session = hush_session_new(0);
    for token in [
        "not-a-token",
        "0.not-a-uuid.secret:AAAAAAAAAAAAAAAAAAAAAA==",
    ] {
        let request = serde_json::json!({"op": "login", "token": token}).to_string();
        assert_eq!(
            code(&call(session, request.as_bytes())),
            Some("invalid_token"),
            "{token}"
        );
    }
    unsafe { hush_session_drop(session) };
}

#[test]
fn a_panic_poisons_the_session() {
    let settings = settings_for(0).expect("US region");
    let mut session = HushSession::open(settings, Duration::from_secs(1)).expect("session");
    let outcome = guarded(&mut session, |_| panic!("simulated SDK panic"));
    assert_eq!(outcome.err(), Some(crate::response::ErrorCode::Internal));

    let session = Box::into_raw(Box::new(session));
    let envelope = call(session, br#"{"op":"catalog"}"#);
    assert_eq!(code(&envelope), Some("internal"));
    unsafe { hush_session_drop(session) };
}
