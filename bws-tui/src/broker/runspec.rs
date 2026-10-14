//! Public run types. See ADR 0002 "Types" and "Child run".
//!
//! None of these types holds a secret value. A binding names a secret; the
//! daemon resolves and fetches it.

use super::meta::SecretRef;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;

/// Exact names copied from the client environment into the child.
pub(crate) const ENV_ALLOWLIST: [&str; 8] = [
    "PATH", "HOME", "USER", "LOGNAME", "LANG", "TERM", "TZ", "TMPDIR",
];

/// Name prefixes that never reach a child, whatever the request says.
pub(crate) const ENV_FORBIDDEN_PREFIXES: [&str; 3] = ["BWS_", "LD_", "DYLD_"];

/// Whether `name` may be copied from the client environment into the child.
pub(crate) fn is_passthrough_allowed(name: &str) -> bool {
    !is_forbidden_env(name) && (ENV_ALLOWLIST.contains(&name) || name.starts_with("LC_"))
}

pub(crate) fn is_forbidden_env(name: &str) -> bool {
    ENV_FORBIDDEN_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// A valid environment variable name. Validated again when it crosses the socket.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct EnvName(String);

impl EnvName {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EnvName {
    type Error = anyhow::Error;

    fn try_from(name: String) -> Result<Self> {
        let mut chars = name.chars();
        let valid = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            bail!("'{name}' is not a valid env var name");
        }
        Ok(Self(name))
    }
}

impl From<EnvName> for String {
    fn from(name: EnvName) -> Self {
        name.0
    }
}

/// How one value reaches the child: `Env` sets `ENV`, `Fd` sets `ENV_FD=<n>` on a pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Delivery {
    Env,
    Fd,
}

/// One secret bound into a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Binding {
    pub secret: SecretRef,
    pub env: EnvName,
    pub delivery: Delivery,
}

/// The child variable a binding sets: `ENV` for env delivery, `ENV_FD` for fd delivery.
pub(crate) fn child_var(env: &EnvName, delivery: Delivery) -> String {
    match delivery {
        Delivery::Env => env.as_str().to_owned(),
        Delivery::Fd => format!("{}_FD", env.as_str()),
    }
}

/// Everything the daemon needs to start one child. Holds names, never values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunSpec {
    pub bindings: Vec<Binding>,
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env_passthrough: Vec<(EnvName, OsString)>,
}

/// The account a child runs as: the caller (default) or `hushrun` when policy requires it.
/// Root is never one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunAs {
    Requester,
    HushRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StreamKind {
    Stdout,
    Stderr,
}

/// Signals the client may forward to the child's process group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ForwardSignal {
    Int,
    Term,
    Hup,
    Quit,
}

impl ForwardSignal {
    pub(crate) fn number(self) -> i32 {
        match self {
            Self::Int => libc::SIGINT,
            Self::Term => libc::SIGTERM,
            Self::Hup => libc::SIGHUP,
            Self::Quit => libc::SIGQUIT,
        }
    }

    pub(crate) fn from_number(signal: i32) -> Option<Self> {
        [Self::Int, Self::Term, Self::Hup, Self::Quit]
            .into_iter()
            .find(|candidate| candidate.number() == signal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "number", rename_all = "snake_case")]
pub(crate) enum RunOutcome {
    Exited(i32),
    Signaled(i32),
}

/// Where the client writes masked child output and daemon notices.
pub(crate) trait OutputSink {
    fn stdout(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn stderr(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    /// One daemon notice, such as a short-secret warning. Never holds a value.
    fn notice(&mut self, message: &str) -> std::io::Result<()>;
}
