//! Metadata domain types. These never hold secret values.

use serde::{Deserialize, Serialize};

/// One secret's metadata. Crosses the socket in clear text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SecretMeta {
    pub id: String,
    pub key: String,
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub note: String,
}

impl SecretMeta {
    /// `query` accepts a project name (case-insensitive) or UUID.
    pub(crate) fn in_project(&self, query: &str) -> bool {
        self.project
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(query))
            || self.project_id.as_deref() == Some(query)
    }
}

/// One project's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectMeta {
    pub id: String,
    pub name: String,
}

/// A reference to one secret. Backends fetch only by `Id`; the daemon
/// resolves a `Key` to exactly one id through its catalog first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecretRef {
    Id(String),
    Key {
        key: String,
        project: Option<String>,
    },
}

/// A secret to create. The value travels separately as [`SecretMaterial`].
pub(crate) struct NewSecret {
    pub project_id: String,
    pub key: String,
    pub note: String,
}

/// The full record an update writes. The value travels separately.
pub(crate) struct SecretUpdate {
    pub id: String,
    pub project_id: Option<String>,
    pub key: String,
    pub note: String,
}

/// A failed write, with no provider text. `Rejected` and `Stopped` mean
/// nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteFailure {
    Rejected,
    /// The client left or the request ran out of time before the write started.
    Stopped,
    Uncertain,
}
