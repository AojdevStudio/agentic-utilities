//! Maps SDK errors to stable codes without letting any error text out.
//!
//! `bitwarden-sm` keeps `SecretsManagerError` in a private module, so its
//! variants cannot be matched by name. The bridge reads the derived `Debug`
//! form instead: the outer variant name and, for server answers, the HTTP
//! status. The text is dropped here. Mock server tests pin every class against
//! the locked SDK version.

use std::fmt::Debug;

use crate::response::ErrorCode;

/// What went wrong, independent of the operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The server answered with this HTTP status.
    Status(u16),
    /// The request did not complete: connect, send, or receive failed.
    Transport,
    /// The bridge deadline passed before the SDK finished.
    Timeout,
    /// The identity server refused the token with a structured error.
    IdentityRejected,
    /// The token is malformed.
    BadToken,
    /// A key could not open a cipher string. During login this means the token
    /// key does not match the payload.
    Crypto,
    /// The SDK validator refused the input before sending it.
    Validation,
    /// Anything else, such as an answer the SDK could not decode.
    Unexpected,
}

/// Classifies an SDK error value from its `Debug` form.
pub fn failure(error: &impl Debug) -> Failure {
    let text = format!("{error:?}");
    if let Some(api) = text.strip_prefix("Api(") {
        api_failure(api)
    } else if text.starts_with("IdentityFail(") {
        Failure::IdentityRejected
    } else if text.starts_with("AccessTokenInvalid(") {
        Failure::BadToken
    } else if text.starts_with("Crypto(") {
        Failure::Crypto
    } else if text.starts_with("Validation(") {
        Failure::Validation
    } else {
        Failure::Unexpected
    }
}

/// Classifies the inside of `ApiError` from bitwarden-core.
fn api_failure(api: &str) -> Failure {
    if let Some(rest) = api.strip_prefix("ResponseContent { status: ") {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        return digits.parse().map_or(Failure::Unexpected, Failure::Status);
    }
    let transport = ["Reqwest(", "ReqwestMiddleware(", "Io("];
    if transport.iter().any(|prefix| api.starts_with(prefix)) {
        Failure::Transport
    } else {
        Failure::Unexpected
    }
}

/// Codes for login. A failed login is always safe to try again.
pub fn login_code(failure: Failure) -> ErrorCode {
    match failure {
        Failure::BadToken | Failure::Crypto => ErrorCode::InvalidToken,
        Failure::IdentityRejected | Failure::Status(400 | 401) => ErrorCode::Authentication,
        Failure::Status(_) | Failure::Transport | Failure::Timeout => ErrorCode::Network,
        Failure::Validation | Failure::Unexpected => ErrorCode::Internal,
    }
}

/// Codes for catalog and read. Both are safe to retry.
pub fn read_code(failure: Failure) -> ErrorCode {
    match failure {
        Failure::Status(status) => status_code(status).unwrap_or(ErrorCode::Network),
        Failure::Transport | Failure::Timeout => ErrorCode::Network,
        Failure::IdentityRejected => ErrorCode::Authentication,
        Failure::Validation => ErrorCode::InvalidRequest,
        Failure::BadToken | Failure::Crypto | Failure::Unexpected => ErrorCode::Internal,
    }
}

/// Codes for create. Only a definite refusal gets a definite code. Anything
/// that may have stored the secret is `UnknownCreate`.
pub fn create_code(failure: Failure) -> ErrorCode {
    match failure {
        Failure::Status(status @ 400..=499) => status_code(status).unwrap_or(ErrorCode::Network),
        Failure::Validation => ErrorCode::InvalidRequest,
        Failure::Status(_)
        | Failure::Transport
        | Failure::Timeout
        | Failure::IdentityRejected
        | Failure::BadToken
        | Failure::Crypto
        | Failure::Unexpected => ErrorCode::UnknownCreate,
    }
}

/// Statuses with their own code. Other statuses fall back per operation.
fn status_code(status: u16) -> Option<ErrorCode> {
    match status {
        400 => Some(ErrorCode::InvalidRequest),
        401 => Some(ErrorCode::Authentication),
        403 | 404 => Some(ErrorCode::Permission),
        _ => None,
    }
}
