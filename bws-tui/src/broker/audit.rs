//! Daemon-owned audit log. Names only, never values or tokens.

use super::peercred::PeerCredentials;
use super::runspec::RunAs;
use crate::install::sys::{c_path, check, fstat, openat, owned};
use anyhow::{bail, Context, Result};
use std::fs::{File, Permissions};
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The daemon records every request with kernel peer identity.
pub(crate) trait AuditLog: Send + Sync {
    fn record_catalog(&self, peer: PeerCredentials, project: Option<&str>) -> Result<()>;
    /// Written after resolution and before any fetch, so refused runs are recorded too.
    fn record_run(&self, peer: PeerCredentials, run: &RunAudit<'_>) -> Result<()>;
    /// Written after resolution and before approval, disclosure, or any write,
    /// so a line records an attempt, not a completed operation.
    fn record_human(&self, peer: PeerCredentials, human: &HumanAudit<'_>) -> Result<()>;
}

/// One human request: the operation and the secret it names.
pub(crate) struct HumanAudit<'a> {
    pub verb: &'a str,
    pub key: &'a str,
    pub project: Option<&'a str>,
    pub id: Option<&'a str>,
}

/// One run request. `secrets` pairs each requested key with its resolved id, if any.
pub(crate) struct RunAudit<'a> {
    pub program: &'a str,
    pub run_as: RunAs,
    pub secrets: Vec<(&'a str, Option<&'a str>)>,
}

/// Append-only file log. The daemon owns this file, not the client audit log.
pub(crate) struct FileAuditLog {
    file: Mutex<std::fs::File>,
}

impl FileAuditLog {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let file = open_private(path)
            .with_context(|| format!("failed to open daemon audit log at {}", path.display()))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("failed to inspect daemon audit log at {}", path.display()))?;
        if !metadata.is_file() {
            bail!("daemon audit log must be a regular file");
        }
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

/// Open the log in its own directory, which only the daemon user may write.
/// The directory above it may be shared, as Ubuntu's root:syslog /var/log is,
/// so its members can rename the log dir or put a symlink in its place. The log
/// dir and the file are opened from descriptors without following a symlink,
/// and the log dir is checked on its open descriptor: it must be the daemon
/// user's and owner-only. A missing log dir is created 0700 the same way and
/// checked before use.
fn open_private(path: &Path) -> Result<File> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        bail!("the path names no file");
    };
    let (Some(above), Some(dir_name)) = (dir.parent(), dir.file_name()) else {
        bail!("the log must sit in its own directory");
    };
    let above = match above.as_os_str().is_empty() {
        true => Path::new("."),
        false => above,
    };
    let (dir_name, name) = (
        dir_name.to_str().context("the log dir name is not UTF-8")?,
        name.to_str().context("the log file name is not UTF-8")?,
    );
    let above = File::open(above)?;
    let open_dir = || openat(above.as_fd(), dir_name, libc::O_RDONLY | libc::O_DIRECTORY);
    let dir = match open_dir() {
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
            let raw = c_path(dir_name.as_bytes())?;
            // SAFETY: the name is NUL-terminated and the parent descriptor is open.
            check(unsafe { libc::mkdirat(above.as_raw_fd(), raw.as_ptr(), 0o700) })?;
            open_dir()
        }
        opened => opened,
    }?;
    let stat = fstat(dir.as_fd())?;
    // SAFETY: geteuid takes no arguments and cannot fail.
    if stat.st_uid != unsafe { libc::geteuid() } {
        bail!("its directory must be owned by the daemon user");
    }
    if stat.st_mode as u32 & 0o077 != 0 {
        bail!("its directory must be owner-only, such as 0700");
    }
    let raw = c_path(name.as_bytes())?;
    let flags =
        libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: the name is NUL-terminated and the mode argument matches O_CREAT.
    let fd = owned(unsafe {
        libc::openat(dir.as_raw_fd(), raw.as_ptr(), flags, 0o600 as libc::c_uint)
    })?;
    Ok(File::from(fd))
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

impl FileAuditLog {
    fn append(&self, verb: &str, peer: PeerCredentials, detail: &str) -> Result<()> {
        let line = format!(
            "{} {verb} uid={} gid={} pid={} {detail}\n",
            humantime::format_rfc3339_seconds(SystemTime::now()),
            peer.uid,
            peer.gid,
            peer.pid,
        );
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("audit log lock poisoned"))?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }
}

impl AuditLog for FileAuditLog {
    fn record_catalog(&self, peer: PeerCredentials, project: Option<&str>) -> Result<()> {
        self.append(
            "catalog",
            peer,
            &format!("project={}", sanitize(project.unwrap_or("-"))),
        )
    }

    fn record_run(&self, peer: PeerCredentials, run: &RunAudit<'_>) -> Result<()> {
        let secrets: Vec<String> = run
            .secrets
            .iter()
            .map(|(key, id)| format!("{}={}", sanitize(key), sanitize(id.unwrap_or("unresolved"))))
            .collect();
        let run_as = match run.run_as {
            RunAs::Requester => "requester",
            RunAs::HushRun => "hushrun",
        };
        self.append(
            "run",
            peer,
            &format!(
                "run_as={run_as} program={} secrets={}",
                sanitize(run.program),
                secrets.join(",")
            ),
        )
    }

    fn record_human(&self, peer: PeerCredentials, human: &HumanAudit<'_>) -> Result<()> {
        self.append("human", peer, &Self::human_line(human))
    }
}

impl FileAuditLog {
    pub(crate) fn human_line(human: &HumanAudit<'_>) -> String {
        format!(
            "op={} key={} project={} id={}",
            human.verb,
            sanitize(human.key),
            sanitize(human.project.unwrap_or("-")),
            sanitize(human.id.unwrap_or("new")),
        )
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
