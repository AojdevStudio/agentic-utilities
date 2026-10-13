//! Response shapes and the JSON envelope that crosses the C boundary.

use serde::Serialize;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Stable failure codes. This is the only error detail Swift ever sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    InvalidToken,
    Authentication,
    Permission,
    Network,
    Locked,
    UnknownCreate,
    Internal,
}

/// Success data for each operation.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Data {
    Login(LoginData),
    Catalog(Catalog),
    Secret(SecretDetail),
}

#[derive(Debug, Serialize)]
pub struct LoginData {
    #[serde(rename = "organizationID")]
    pub organization_id: Uuid,
}

/// Metadata only. A catalog never holds a secret value or note.
#[derive(Debug, Serialize)]
pub struct Catalog {
    pub projects: Vec<ProjectSummary>,
    pub secrets: Vec<SecretSummary>,
}

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SecretSummary {
    pub id: Uuid,
    pub key: String,
    #[serde(rename = "projectIDs")]
    pub project_ids: Vec<Uuid>,
}

/// One secret with its value. Value and note are zeroized on drop.
#[derive(Debug, Serialize)]
pub struct SecretDetail {
    pub id: Uuid,
    pub key: String,
    pub value: Zeroizing<String>,
    pub note: Zeroizing<String>,
    #[serde(rename = "projectID")]
    pub project_id: Option<Uuid>,
}

#[derive(Serialize)]
struct Success<'a> {
    ok: bool,
    data: &'a Data,
}

#[derive(Serialize)]
struct Failure {
    ok: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: ErrorCode,
}

/// Fallback when serialization itself fails. It never should.
const INTERNAL_ENVELOPE: &[u8] = br#"{"ok":false,"error":{"code":"internal"}}"#;

/// Encodes one outcome as the JSON envelope Swift decodes.
pub fn encode(outcome: Result<Data, ErrorCode>) -> Zeroizing<Vec<u8>> {
    let bytes = match &outcome {
        Ok(data) => serde_json::to_vec(&Success { ok: true, data }),
        Err(code) => serde_json::to_vec(&Failure {
            ok: false,
            error: ErrorBody { code: *code },
        }),
    };
    Zeroizing::new(bytes.unwrap_or_else(|_| INTERNAL_ENVELOPE.to_vec()))
}
