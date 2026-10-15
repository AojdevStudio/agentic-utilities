//! Daemon token custody checks. The token value never enters an error.

use super::hardening::check_token_metadata;
use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use zeroize::Zeroizing;

/// Largest token accepted from a file or from install input.
pub(crate) const TOKEN_MAX_BYTES: usize = 64 * 1024;

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
    read_secret(&mut file).with_context(|| format!("token file at {}", path.display()))
}

/// Read one token under the shared bound. Every buffer is zeroized and no
/// error carries input bytes, so a failure cannot echo part of a token.
pub(crate) fn read_secret(reader: &mut impl Read) -> Result<SecretString> {
    let mut raw = Zeroizing::new(Vec::with_capacity(TOKEN_MAX_BYTES));
    let mut chunk = Zeroizing::new([0u8; 4096]);
    loop {
        let read = match reader.read(&mut chunk[..]) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => bail!("failed to read the token"),
        };
        if raw.len() + read > TOKEN_MAX_BYTES {
            bail!("token is larger than 64 KiB");
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    secret_from_bytes(&raw)
}

/// Check token bytes and copy the trimmed text into exactly sized storage,
/// so building the secret never reallocates and leaves a copy behind.
pub(crate) fn secret_from_bytes(raw: &[u8]) -> Result<SecretString> {
    if raw.contains(&0) {
        bail!("token contains a NUL byte");
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        bail!("token is not valid UTF-8");
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        bail!("token is empty");
    }
    let mut owned = String::with_capacity(trimmed.len());
    owned.push_str(trimmed);
    Ok(SecretString::from(owned))
}

/// Open `path` read-only without following a final symlink. std adds
/// `O_CLOEXEC`. `O_NONBLOCK` keeps a FIFO at this path from blocking the open.
fn open_nofollow(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("token file missing at {}", path.display()))
}
