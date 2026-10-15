//! Where an install puts each artifact. One table describes every managed
//! path for inspection, publication, and reporting.

use super::model::Backend;
use crate::broker::{audit, paths};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// The one name shared by encryption, the unit, and the runtime path.
pub(crate) const CREDENTIAL_NAME: &str = "bws-token";

/// The managed tree and the owner every protected object must have.
/// Production is the real root and uid 0. Tests use a private directory and
/// their own uid, so the same checks run on real files without root.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub anchor: PathBuf,
    pub owner: u32,
}

impl Layout {
    pub(crate) fn production() -> Self {
        Self {
            anchor: PathBuf::from("/"),
            owner: 0,
        }
    }

    /// Components below the anchor. macOS reaches /etc and /var through
    /// root-owned aliases into /private, so those are walked directly.
    pub(crate) fn components(&self, logical: &Path) -> Vec<OsString> {
        let mut parts: Vec<OsString> = logical
            .components()
            .filter_map(|part| match part {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        if cfg!(target_os = "macos")
            && parts
                .first()
                .is_some_and(|first| first == "etc" || first == "var")
        {
            parts.insert(0, OsString::from("private"));
        }
        parts
    }

    /// Path to show or connect to. Never used to open a protected object.
    pub(crate) fn display(&self, logical: &Path) -> PathBuf {
        self.components(logical)
            .iter()
            .fold(self.anchor.clone(), |path, part| path.join(part))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Dir,
    /// Root-only token source. Install never reads it back.
    Token(Backend),
    /// Exact bytes install renders.
    File(Vec<u8>),
    /// Rendered when absent. A safe existing service is operator policy.
    Pam(Vec<u8>),
}

impl Kind {
    pub(crate) fn mode(&self) -> u32 {
        match self {
            Self::Dir => 0o700,
            Self::Token(_) => 0o600,
            Self::File(_) | Self::Pam(_) => 0o644,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spec {
    pub path: PathBuf,
    pub kind: Kind,
}

/// Directories and the token source, in creation order. A directory shared
/// by the token and the audit log appears once.
pub(crate) fn private_specs(backend: Backend) -> Vec<Spec> {
    let token = token_source(backend);
    let mut dirs: Vec<PathBuf> = Vec::new();
    for file in [&token, &audit::fixed_audit_path()] {
        if let Some(dir) = file.parent() {
            if !dirs.iter().any(|known| known == dir) {
                dirs.push(dir.to_path_buf());
            }
        }
    }
    let mut specs: Vec<Spec> = dirs
        .into_iter()
        .map(|path| Spec {
            path,
            kind: Kind::Dir,
        })
        .collect();
    specs.push(Spec {
        path: token,
        kind: Kind::Token(backend),
    });
    specs
}

/// The token source the installer writes for this backend.
pub(crate) fn token_source(backend: Backend) -> PathBuf {
    match backend {
        Backend::Encrypted => paths::default_token_path().with_extension("cred"),
        Backend::LoadCredential | Backend::File => paths::default_token_path(),
    }
}

/// The token path the daemon opens at runtime.
pub(crate) fn runtime_token(backend: Backend) -> String {
    match backend {
        Backend::Encrypted | Backend::LoadCredential => {
            format!("${{CREDENTIALS_DIRECTORY}}/{CREDENTIAL_NAME}")
        }
        Backend::File => paths::default_token_path().display().to_string(),
    }
}

pub(crate) fn definition_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/LaunchDaemons/hush.plist")
    } else {
        PathBuf::from("/etc/systemd/system/hush.service")
    }
}

/// Where systemd keeps the key `systemd-creds setup` creates.
pub(crate) fn host_key_path() -> PathBuf {
    PathBuf::from("/var/lib/systemd/credential.secret")
}

/// Fixed socket path. Install never honors the debug override.
pub(crate) fn socket_path() -> PathBuf {
    paths::socket_path(None, paths::Build::Release)
}
