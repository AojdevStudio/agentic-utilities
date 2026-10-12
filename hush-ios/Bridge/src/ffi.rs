//! The four C functions. Every entry point runs inside `catch_unwind`, so no
//! panic crosses the boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{ptr, slice};

use bitwarden::secrets_manager::ClientSettings;
use std::time::Duration;
use zeroize::{Zeroize, Zeroizing};

use crate::request::{self, MAX_REQUEST_BYTES};
use crate::response::{Data, ErrorCode, encode};
use crate::session::{self, Session};

/// Every network operation finishes or fails within this deadline.
const DEADLINE: Duration = Duration::from_secs(30);

/// Opaque to C. Owns one SDK session and its poison flag.
pub struct HushSession {
    session: Session,
    poisoned: bool,
}

/// Bytes owned by the bridge until `hush_buffer_free`.
#[repr(C)]
pub struct HushBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl HushSession {
    /// Opens a session against explicit endpoints. Tests use local mock servers.
    pub(crate) fn open(settings: ClientSettings, deadline: Duration) -> Option<Self> {
        let session = Session::new(settings, deadline).ok()?;
        Some(Self {
            session,
            poisoned: false,
        })
    }
}

/// Creates a session for region 0 (US) or 1 (EU). Returns NULL otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn hush_session_new(region: u32) -> *mut HushSession {
    catch_unwind(|| {
        let settings = session::settings_for(region)?;
        HushSession::open(settings, DEADLINE)
    })
    .ok()
    .flatten()
    .map_or(ptr::null_mut(), |session| Box::into_raw(Box::new(session)))
}

/// Runs one JSON request and returns one JSON envelope.
///
/// # Safety
///
/// `session` is NULL or a live pointer from `hush_session_new`, used by one
/// thread at a time. `input` is NULL or points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hush_session_call(
    session: *mut HushSession,
    input: *const u8,
    len: usize,
) -> HushBuffer {
    let envelope = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the caller contract above makes both pointers valid or NULL.
        let session = unsafe { session.as_mut() };
        let input = unsafe { copy_input(input, len) };
        encode(call(session, input))
    }))
    .unwrap_or_else(|_| encode(Err(ErrorCode::Internal)));
    into_buffer(&envelope)
}

/// Destroys a session. NULL is a no-op.
///
/// # Safety
///
/// `session` is NULL or a live pointer from `hush_session_new` that no other
/// call is using. The pointer is invalid afterward.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hush_session_drop(session: *mut HushSession) {
    if session.is_null() {
        return;
    }
    // SAFETY: the pointer came from Box::into_raw in hush_session_new.
    let session = unsafe { Box::from_raw(session) };
    // A panic while dropping SDK state is contained here, never unwound into C.
    let _ = catch_unwind(AssertUnwindSafe(move || drop(session)));
}

/// Zeroizes and frees a buffer from `hush_session_call`.
///
/// # Safety
///
/// `buffer` came from `hush_session_call` and was not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hush_buffer_free(buffer: HushBuffer) {
    if buffer.data.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: data and len describe the boxed slice built by into_buffer.
        let mut bytes =
            unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) };
        bytes.zeroize();
    }));
}

/// Parses and runs one request, poisoning the session if the SDK panics.
fn call(
    session: Option<&mut HushSession>,
    input: Option<Zeroizing<Vec<u8>>>,
) -> Result<Data, ErrorCode> {
    let session = session.ok_or(ErrorCode::InvalidRequest)?;
    if session.poisoned {
        return Err(ErrorCode::Internal);
    }
    let request = request::parse(&input.ok_or(ErrorCode::InvalidRequest)?)?;
    guarded(session, |inner| inner.handle(request))
}

/// Runs `work` and poisons the session when it panics.
pub(crate) fn guarded(
    session: &mut HushSession,
    work: impl FnOnce(&mut Session) -> Result<Data, ErrorCode>,
) -> Result<Data, ErrorCode> {
    let outcome = catch_unwind(AssertUnwindSafe(|| work(&mut session.session)));
    outcome.unwrap_or_else(|_| {
        session.poisoned = true;
        Err(ErrorCode::Internal)
    })
}

/// Copies caller bytes. NULL, empty, or oversized input yields `None`.
///
/// # Safety
///
/// `input` is NULL or points to `len` readable bytes.
unsafe fn copy_input(input: *const u8, len: usize) -> Option<Zeroizing<Vec<u8>>> {
    if input.is_null() || len == 0 || len > MAX_REQUEST_BYTES {
        return None;
    }
    // SAFETY: checked non-null above; the caller guarantees `len` bytes.
    let bytes = unsafe { slice::from_raw_parts(input, len) };
    Some(Zeroizing::new(bytes.to_vec()))
}

/// Moves an envelope into an exact-size allocation that C can hold.
fn into_buffer(envelope: &[u8]) -> HushBuffer {
    let boxed: Box<[u8]> = Box::from(envelope);
    let len = boxed.len();
    HushBuffer {
        data: Box::into_raw(boxed).cast::<u8>(),
        len,
    }
}
