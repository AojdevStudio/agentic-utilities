//! Request parsing and limits. Everything here runs before any network call.

use serde::Deserialize;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::response::ErrorCode;

/// Largest request the bridge copies. A 25000 character value fully escaped
/// as JSON stays well under this.
pub const MAX_REQUEST_BYTES: usize = 512 * 1024;
const MAX_TOKEN_BYTES: usize = 1024;
// Character limits match the SDK validator on SecretCreateRequest.
const MAX_KEY_CHARS: usize = 500;
const MAX_VALUE_CHARS: usize = 25_000;
const MAX_NOTE_CHARS: usize = 7_000;

/// One validated request.
#[derive(Debug)]
pub enum Request {
    Login { token: Zeroizing<String> },
    Catalog,
    Read { id: Uuid },
    Create(NewSecret),
}

/// A validated create request. Value and note are zeroized on drop.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewSecret {
    pub key: String,
    pub value: Zeroizing<String>,
    pub note: Zeroizing<String>,
    #[serde(rename = "projectID")]
    pub project_id: Uuid,
}

/// The wire form. `op` selects the variant and unknown fields are rejected.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Wire {
    Login { token: Zeroizing<String> },
    // A struct variant, so deny_unknown_fields also covers catalog.
    Catalog {},
    Read { id: Uuid },
    Create(NewSecret),
}

/// Parses UTF-8 JSON into a request and enforces every limit.
pub fn parse(bytes: &[u8]) -> Result<Request, ErrorCode> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(ErrorCode::InvalidRequest);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ErrorCode::InvalidRequest)?;
    let wire: Wire = serde_json::from_str(text).map_err(|_| ErrorCode::InvalidRequest)?;
    let request = match wire {
        Wire::Login { token } => Request::Login { token },
        Wire::Catalog {} => Request::Catalog,
        Wire::Read { id } => Request::Read { id },
        Wire::Create(secret) => Request::Create(secret),
    };
    if within_limits(&request) {
        Ok(request)
    } else {
        Err(ErrorCode::InvalidRequest)
    }
}

fn within_limits(request: &Request) -> bool {
    match request {
        Request::Login { token } => (1..=MAX_TOKEN_BYTES).contains(&token.len()),
        Request::Catalog | Request::Read { .. } => true,
        Request::Create(secret) => {
            fits(&secret.key, 1, MAX_KEY_CHARS)
                && !is_blank(&secret.key)
                && fits(&secret.value, 1, MAX_VALUE_CHARS)
                && fits(&secret.note, 0, MAX_NOTE_CHARS)
                && (secret.note.is_empty() || !is_blank(&secret.note))
        }
    }
}

/// Counts Unicode scalar values, the unit the SDK validator uses.
fn fits(text: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&text.chars().count())
}

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}
