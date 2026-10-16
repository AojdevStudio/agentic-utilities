//! Fixed install paths and names. Production paths are absolute below `/`.
//! Tests root the same logical paths in a private directory, so there is no
//! release-build escape hatch to another root.

use super::custody::Owner;
use crate::broker::paths::{default_socket_path, DEFAULT_BWS_CANDIDATES};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// One credential name for encryption, the unit and the runtime path.
pub(crate) const CREDENTIAL_NAME: &str = "bws-token";
pub(crate) const UNIT_NAME: &str = "hush.service";
/// Proposed internal launchd label. It is not a published reverse-DNS identity.
pub(crate) const LAUNCHD_LABEL: &str = "hush";
pub(crate) const HOST_KEY: &str = "/var/lib/systemd/credential.secret";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Linux,
    MacOs,
}

/// How the daemon receives its token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Backend {
    /// systemd 250 or newer: `LoadCredentialEncrypted=` from host-key ciphertext.
    Encrypted,
    /// systemd 247 to 249: `LoadCredential=` from a root-only 0600 file.
    LoadCredential,
    /// macOS root LaunchDaemon reading a root-only 0600 file.
    Launchd,
}

pub(crate) struct Layout {
    pub root: PathBuf,
    pub owner: Owner,
    pub platform: Platform,
    pub executable: PathBuf,
    pub bws_candidates: Vec<PathBuf>,
    pub socket: PathBuf,
}

impl Layout {
    pub(crate) fn production() -> Result<Self> {
        let executable = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .context("cannot resolve the running hush executable")?;
        Ok(Self {
            root: PathBuf::from("/"),
            owner: Owner(0),
            platform: if cfg!(target_os = "macos") {
                Platform::MacOs
            } else {
                Platform::Linux
            },
            executable,
            bws_candidates: DEFAULT_BWS_CANDIDATES.iter().map(PathBuf::from).collect(),
            socket: default_socket_path(),
        })
    }

    /// The production logical paths, rooted in a private test directory.
    #[cfg(test)]
    pub(crate) fn in_root(root: &Path, owner: Owner, platform: Platform) -> Self {
        let socket = match platform {
            Platform::Linux => "/run/hush/hush.sock",
            Platform::MacOs => "/var/run/hush/hush.sock",
        };
        Self {
            root: root.to_path_buf(),
            owner,
            platform,
            executable: PathBuf::from("/usr/local/bin/hush"),
            bws_candidates: vec![PathBuf::from("/usr/local/bin/bws")],
            socket: PathBuf::from(socket),
        }
    }

    /// Where a logical path lives on this host.
    pub(crate) fn host_path(&self, logical: &Path) -> PathBuf {
        self.root.join(rel(logical))
    }

    pub(crate) fn token_dir(&self) -> &'static str {
        match self.platform {
            Platform::Linux => "/etc/hush",
            Platform::MacOs => "/private/var/db/hush",
        }
    }

    /// A separate audit directory. macOS keeps the audit log beside the token.
    pub(crate) fn log_dir(&self) -> Option<&'static str> {
        match self.platform {
            Platform::Linux => Some("/var/log/hush"),
            Platform::MacOs => None,
        }
    }

    pub(crate) fn audit_log(&self) -> String {
        format!("{}/audit.log", self.log_dir().unwrap_or(self.token_dir()))
    }

    pub(crate) fn credential(&self, backend: Backend) -> String {
        let name = if backend == Backend::Encrypted {
            "token.cred"
        } else {
            "token"
        };
        format!("{}/{name}", self.token_dir())
    }

    pub(crate) fn policy(&self) -> &'static str {
        match self.platform {
            Platform::Linux => "/etc/hush/hushd.toml",
            Platform::MacOs => "/private/etc/hush/hushd.toml",
        }
    }

    pub(crate) fn service_file(&self) -> &'static str {
        match self.platform {
            Platform::Linux => "/etc/systemd/system/hush.service",
            Platform::MacOs => "/Library/LaunchDaemons/hush.plist",
        }
    }

    /// The daemon command line, shared by the unit, the plist and the
    /// manager readback. Paths are restricted to characters that need no
    /// quoting in either format.
    pub(crate) fn daemon_args(&self, backend: Backend, bws: &Path) -> Result<Vec<String>, String> {
        let token = match backend {
            Backend::Launchd => self.credential(backend),
            Backend::Encrypted | Backend::LoadCredential => {
                format!("${{CREDENTIALS_DIRECTORY}}/{CREDENTIAL_NAME}")
            }
        };
        let audit = self.audit_log();
        let paths = [
            &self.executable,
            &self.socket,
            Path::new(&audit),
            bws,
            Path::new(self.policy()),
        ];
        if let Some(bad) = paths.iter().find(|path| !plain(path)) {
            return Err(format!(
                "{} cannot be written safely into a service definition",
                bad.display()
            ));
        }
        let text = |path: &Path| path.display().to_string();
        Ok(vec![
            text(&self.executable),
            "daemon".into(),
            "--socket".into(),
            text(&self.socket),
            "--token-file".into(),
            token,
            "--audit-log".into(),
            audit,
            "--bws-path".into(),
            text(bws),
            "--policy".into(),
            self.policy().into(),
        ])
    }
}

/// A logical absolute path relative to the layout root.
pub(crate) fn rel(path: &Path) -> &Path {
    path.strip_prefix("/").unwrap_or(path)
}

fn plain(path: &Path) -> bool {
    path.is_absolute()
        && path.to_str().is_some_and(|text| {
            text.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/._+-".contains(&byte))
        })
}
