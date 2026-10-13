//! Daemon token custody checks. The token value never enters an error.

use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Pure check on observed metadata so tests can cover owner refusal without `chown`.
pub(crate) fn check_metadata(mode: u32, owner_uid: u32, euid: u32) -> Result<()> {
    if mode & 0o777 != 0o600 {
        bail!("token file must have mode 0600");
    }
    // Production owns by root. Same-user development owns by the daemon user.
    // Task contract: not owned by the daemon's own effective uid is refused.
    if owner_uid != euid {
        bail!("token file must be owned by the daemon user");
    }
    Ok(())
}

/// Read the token only if the file is mode 0600 and owned by this effective uid.
pub(crate) fn load_checked(path: &Path) -> Result<SecretString> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("token file missing at {}", path.display()))?;
    check_metadata(metadata.mode(), metadata.uid(), unsafe { libc::geteuid() })?;
    let raw = std::fs::read(path)
        .with_context(|| format!("failed to read token file at {}", path.display()))?;
    let text = String::from_utf8(raw)?.trim().to_string();
    if text.is_empty() {
        bail!("token file is empty");
    }
    Ok(SecretString::from(text))
}
