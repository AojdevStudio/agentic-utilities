//! Daemon token custody checks. The token value never enters an error.

use super::hardening::check_token_metadata;
use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// Read the token from one open file description that passed every check.
/// The checks run on that same descriptor, so the path cannot be swapped between check and read.
pub(crate) fn load_checked(path: &Path, dev_same_user: bool) -> Result<SecretString> {
    let mut file = open_nofollow(path)?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to stat token file at {}", path.display()))?;
    if !metadata.is_file() {
        bail!("token file must be a regular file");
    }
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    check_token_metadata(metadata.mode(), metadata.uid(), euid, dev_same_user)?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .with_context(|| format!("failed to read token file at {}", path.display()))?;
    let text = String::from_utf8(raw)?.trim().to_string();
    if text.is_empty() {
        bail!("token file is empty");
    }
    Ok(SecretString::from(text))
}

/// Open `path` read-only without following a final symlink. std adds `O_CLOEXEC`.
fn open_nofollow(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .with_context(|| format!("token file missing at {}", path.display()))
}
