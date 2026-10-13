//! Daemon-owned audit log. Names only, never values or tokens.

use super::peercred::PeerCredentials;
use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The daemon records every catalog request with kernel peer identity.
pub(crate) trait AuditLog: Send + Sync {
    fn record_catalog(&self, peer: PeerCredentials, project: Option<&str>) -> Result<()>;
}

/// Append-only file log. The daemon owns this file, not the client audit log.
pub(crate) struct FileAuditLog {
    file: Mutex<std::fs::File>,
}

impl FileAuditLog {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("failed to open daemon audit log at {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl AuditLog for FileAuditLog {
    fn record_catalog(&self, peer: PeerCredentials, project: Option<&str>) -> Result<()> {
        let line = format!(
            "{} catalog uid={} gid={} pid={} project={}\n",
            humantime::format_rfc3339_seconds(SystemTime::now()),
            peer.uid,
            peer.gid,
            peer.pid,
            project.unwrap_or("-"),
        );
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("audit log lock poisoned"))?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }
}

/// Default daemon audit path. The `daemon` subcommand can override it.
pub(crate) fn default_audit_path() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSH_AUDIT_LOG") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/db/hush/audit.log")
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from("/var/log/hush/audit.log")
    }
}
