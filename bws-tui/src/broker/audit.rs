//! Daemon-owned audit log. Names only, never values or tokens.

use super::peercred::PeerCredentials;
use anyhow::{bail, Context, Result};
use std::fs::{OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
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
            .mode(0o600)
            .open(path)
            .with_context(|| format!("failed to open daemon audit log at {}", path.display()))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("failed to inspect daemon audit log at {}", path.display()))?;
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        if metadata.uid() != euid {
            bail!("daemon audit log must be owned by the daemon user");
        }
        if metadata.mode() & 0o077 != 0 {
            bail!("daemon audit log must not be group or world readable or writable");
        }
        // A created file can still lose bits to umask; pin 0600 for every daemon run.
        file.set_permissions(Permissions::from_mode(0o600))
            .with_context(|| format!("failed to secure daemon audit log at {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

/// Escape client-controlled text so one record stays exactly one line.
/// Newlines, carriage returns, other control bytes, and backslash become
/// escapes, so injected lines cannot look like daemon-authored records.
fn sanitize(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

impl AuditLog for FileAuditLog {
    fn record_catalog(&self, peer: PeerCredentials, project: Option<&str>) -> Result<()> {
        let line = format!(
            "{} catalog uid={} gid={} pid={} project={}\n",
            humantime::format_rfc3339_seconds(SystemTime::now()),
            peer.uid,
            peer.gid,
            peer.pid,
            sanitize(project.unwrap_or("-")),
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
