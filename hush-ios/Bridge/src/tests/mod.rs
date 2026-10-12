//! Tests drive the four C functions the way the Swift caller does.

mod boundary;
mod cloud;
mod sdk;

use serde_json::Value;

use crate::ffi::{HushBuffer, HushSession, hush_buffer_free, hush_session_call};

/// Decodes and frees one returned envelope.
fn take(buffer: HushBuffer) -> Value {
    assert!(!buffer.data.is_null());
    // SAFETY: the buffer came from hush_session_call and is freed once below.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) }.to_vec();
    unsafe { hush_buffer_free(buffer) };
    serde_json::from_slice(&bytes).expect("envelope is JSON")
}

/// Sends one request through the C ABI.
fn call(session: *mut HushSession, request: &[u8]) -> Value {
    // SAFETY: tests pass live sessions and a readable slice.
    take(unsafe { hush_session_call(session, request.as_ptr(), request.len()) })
}

fn code(envelope: &Value) -> Option<&str> {
    envelope["error"]["code"].as_str()
}
