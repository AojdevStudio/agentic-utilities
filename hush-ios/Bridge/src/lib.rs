//! Narrow C bridge from the Hush iOS app to the Bitwarden Secrets Manager SDK.
//!
//! The C contract lives in `contract.md` and `include/hush_bridge.h`. Only the
//! four functions re-exported here cross the C boundary.

mod classify;
mod ffi;
mod request;
mod response;
mod session;
#[cfg(test)]
mod tests;

pub use ffi::{
    HushBuffer, HushSession, hush_buffer_free, hush_session_call, hush_session_drop,
    hush_session_new,
};
