//! Typed human operations on the private wire. No variant can hold a secret
//! value or a password. Values enter the daemon through a passed descriptor.

use super::meta::SecretMeta;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Where the daemon reads a new value from. Never the value itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ValueSource {
    /// A passed pipe or file. The daemon reads it to end of file.
    InputFd,
    /// The passed terminal. The daemon prompts with echo off.
    TtyPrompt,
}

/// An edit keeps the stored value unless it names a source for a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ValueChange {
    Keep,
    Replace(ValueSource),
}

/// One existing secret, by id or by key with an optional project filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecretSelector {
    Id {
        id: String,
    },
    Key {
        key: String,
        project: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Operation {
    /// Creates a secret. Refused when the key already exists in the project.
    Add {
        project: String,
        key: String,
        note: Option<String>,
        value: ValueSource,
    },
    Reveal {
        secret: SecretSelector,
    },
    Copy {
        secret: SecretSelector,
    },
    Edit {
        secret: SecretSelector,
        new_key: Option<String>,
        note: Option<String>,
        value: ValueChange,
    },
    Delete {
        secret: SecretSelector,
    },
}

/// What one passed descriptor is for, in the order the client sends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FdRole {
    Terminal,
    ValueInput,
}

/// Descriptor roles and approval for one operation. Every policy decision
/// about a variant lives in [`Operation::contract`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Contract {
    pub roles: &'static [FdRole],
    pub approval: bool,
}

impl Operation {
    pub(crate) fn contract(&self) -> Contract {
        use FdRole::{Terminal, ValueInput};
        let (roles, approval): (&'static [FdRole], bool) = match self {
            Self::Add {
                value: ValueSource::InputFd,
                ..
            } => (&[ValueInput], false),
            Self::Add {
                value: ValueSource::TtyPrompt,
                ..
            } => (&[Terminal], false),
            Self::Reveal { .. } | Self::Copy { .. } | Self::Delete { .. } => (&[Terminal], true),
            Self::Edit {
                value: ValueChange::Keep | ValueChange::Replace(ValueSource::TtyPrompt),
                ..
            } => (&[Terminal], true),
            Self::Edit {
                value: ValueChange::Replace(ValueSource::InputFd),
                ..
            } => (&[Terminal, ValueInput], true),
        };
        Contract { roles, approval }
    }

    /// The audit and banner verb.
    pub(crate) fn verb(&self) -> &'static str {
        match self {
            Self::Add { .. } => "add",
            Self::Reveal { .. } => "reveal",
            Self::Copy { .. } => "copy",
            Self::Edit { .. } => "edit",
            Self::Delete { .. } => "delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub(crate) enum Outcome {
    Added {
        meta: SecretMeta,
    },
    Edited {
        meta: SecretMeta,
    },
    Deleted {
        id: String,
        key: String,
    },
    Revealed {
        key: String,
    },
    /// Written to the terminal as OSC 52. The terminal cannot confirm delivery.
    CopySent {
        key: String,
    },
}

/// Why a passed descriptor set was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DescriptorProblem {
    /// Rights arrived truncated, after the first byte, or as a foreign control message.
    Malformed,
    WrongCount {
        expected: u8,
        received: u8,
    },
    NotATerminal,
    NotReadWrite,
    WrongOwner,
    /// The terminal is not the requesting process's controlling terminal.
    NotControlling,
    /// The requesting process is not in the terminal's foreground process group.
    NotForeground,
    /// A value input must be a pipe or file, not a terminal.
    ValueInputIsTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub(crate) enum Refusal {
    NotFound {
        key: String,
    },
    Ambiguous {
        key: String,
        count: u32,
    },
    ProjectNotFound {
        query: String,
    },
    ProjectAmbiguous {
        query: String,
    },
    AlreadyExists {
        key: String,
        id: String,
    },
    Descriptors {
        problem: DescriptorProblem,
    },
    ApprovalDenied,
    ApprovalCancelled,
    ApprovalUnavailable {
        reason: String,
    },
    InvalidValue {
        reason: String,
    },
    VaultFailed {
        operation: String,
    },
    /// The provider call did not finish cleanly. The vault may or may not hold the change.
    WriteUncertain {
        operation: String,
    },
    NothingToChange,
    /// The daemon audit log could not record the request, so nothing ran.
    AuditFailed,
    /// The terminal stopped working mid-operation.
    TerminalFailed,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { key } => write!(f, "no secret with key '{key}'"),
            Self::Ambiguous { key, count } => write!(
                f,
                "{count} secrets are named '{key}'. Narrow with --project"
            ),
            Self::ProjectNotFound { query } => write!(f, "no project named '{query}'"),
            Self::ProjectAmbiguous { query } => {
                write!(f, "ambiguous project name '{query}'. Use the UUID")
            }
            Self::AlreadyExists { key, id } => write!(
                f,
                "'{key}' already exists in that project ({id}). Use hush edit to replace its value"
            ),
            Self::Descriptors { problem } => write!(f, "refused terminal or input: {problem:?}"),
            Self::ApprovalDenied => write!(f, "approval denied"),
            Self::ApprovalCancelled => write!(f, "approval cancelled"),
            Self::ApprovalUnavailable { reason } => write!(f, "approval unavailable: {reason}"),
            Self::InvalidValue { reason } => write!(f, "invalid value: {reason}"),
            Self::VaultFailed { operation } => write!(f, "vault {operation} failed"),
            Self::WriteUncertain { operation } => write!(
                f,
                "vault {operation} did not finish cleanly. The change may or may not exist. \
Check `hush list` before trying again"
            ),
            Self::NothingToChange => {
                write!(f, "nothing to change. Pass --new-key, --note, or --stdin")
            }
            Self::AuditFailed => write!(f, "the daemon audit log failed, so nothing ran"),
            Self::TerminalFailed => write!(f, "the terminal stopped responding"),
        }
    }
}
