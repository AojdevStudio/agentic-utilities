//! Socket path resolution and trusted bws path checks.

use super::hardening::{check_not_group_or_world_writable, check_owner};
use anyhow::{bail, Context, Result};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Absolute bws candidates checked when the daemon gets no `--bws-path`.
pub(crate) const DEFAULT_BWS_CANDIDATES: [&str; 3] = [
    "/usr/local/bin/bws",
    "/opt/homebrew/bin/bws",
    "/usr/bin/bws",
];

/// Socket path. ADR 0002 fixes the default. `HUSH_SOCKET` overrides it.
pub(crate) fn default_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSH_SOCKET") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/run/hush/hush.sock")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/run/hush/hush.sock")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        PathBuf::from("hush.sock")
    }
}

/// Default token location. The `daemon` subcommand can override it.
pub(crate) fn default_token_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/db/hush/token")
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from("/etc/hush/token")
    }
}

/// Resolve and validate the bws binary the root daemon will run.
/// The daemon never consults `PATH`.
pub(crate) fn resolve_trusted_bws(
    explicit: Option<PathBuf>,
    dev_same_user: bool,
) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return validate_trusted_bws_path(&path, dev_same_user);
    }
    let mut last_error: Option<anyhow::Error> = None;
    for candidate in DEFAULT_BWS_CANDIDATES {
        match validate_trusted_bws_path(Path::new(candidate), dev_same_user) {
            Ok(canonical) => return Ok(canonical),
            Err(error) => last_error = Some(error),
        }
    }
    match last_error {
        Some(error) => Err(error).context("no trusted bws binary found"),
        None => bail!("no trusted bws binary found"),
    }
}

/// Validate one bws path and return its canonical form.
pub(crate) fn validate_trusted_bws_path(path: &Path, dev_same_user: bool) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("bws path must be absolute");
    }
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("bws binary missing at {}", path.display()))?;
    let metadata = std::fs::metadata(&canonical)
        .with_context(|| format!("bws binary missing at {}", canonical.display()))?;
    if !metadata.is_file() {
        bail!("bws path must be a regular file");
    }
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    check_owner(metadata.uid(), euid, dev_same_user)
        .with_context(|| format!("bws binary at {}", canonical.display()))?;
    check_not_group_or_world_writable(metadata.mode())
        .with_context(|| format!("bws binary at {}", canonical.display()))?;
    let parent = canonical
        .parent()
        .context("bws binary has no parent directory")?;
    let parent_meta = std::fs::metadata(parent)
        .with_context(|| format!("bws parent directory missing at {}", parent.display()))?;
    if !parent_meta.is_dir() {
        bail!("bws parent path must be a directory");
    }
    check_owner(parent_meta.uid(), euid, dev_same_user)
        .with_context(|| format!("bws parent directory at {}", parent.display()))?;
    check_not_group_or_world_writable(parent_meta.mode())
        .with_context(|| format!("bws parent directory at {}", parent.display()))?;
    Ok(canonical)
}
