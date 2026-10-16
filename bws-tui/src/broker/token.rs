//! Daemon token custody checks. The token value never enters an error.

use super::hardening::check_token_metadata;
use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use zeroize::Zeroizing;

/// Largest token the loader and the installer accept, in bytes.
pub(crate) const TOKEN_LIMIT: usize = 64 * 1024;

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
    read_bounded(&mut file).with_context(|| format!("token file at {}", path.display()))
}

/// Read and parse at most `TOKEN_LIMIT` bytes. Every buffer is zeroed on drop,
/// and every error is fixed text, so no input byte outlives this call.
pub(crate) fn read_bounded(reader: &mut impl Read) -> Result<SecretString> {
    // One allocation up front, so no growth leaves an unzeroed copy behind.
    let mut raw = Zeroizing::new(vec![0u8; TOKEN_LIMIT + 1]);
    let mut filled = 0;
    while filled < raw.len() {
        match reader.read(&mut raw[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => bail!("failed to read the token"),
        }
    }
    if filled > TOKEN_LIMIT {
        bail!("token is larger than {TOKEN_LIMIT} bytes");
    }
    let raw = &raw[..filled];
    if raw.contains(&0) {
        bail!("token contains a NUL byte");
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        bail!("token is not valid UTF-8");
    };
    let text = text.trim();
    if text.is_empty() {
        bail!("token is empty");
    }
    Ok(SecretString::from(text.to_owned()))
}

/// Open `path` read-only without following a final symlink. std adds `O_CLOEXEC`.
/// O_NONBLOCK keeps a FIFO at this path from blocking the open before the type check.
fn open_nofollow(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("token file missing at {}", path.display()))
}
