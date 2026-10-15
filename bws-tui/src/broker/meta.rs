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
