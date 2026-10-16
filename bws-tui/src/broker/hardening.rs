//! Pure ownership and mode checks shared by the token file and the bws binary.

use anyhow::{bail, Result};

/// Token modes the daemon loads, special bits included. systemd hands a Linux
/// runtime credential over read-only at 0400. macOS keeps exactly 0600.
#[cfg(target_os = "linux")]
const TOKEN_MODES: (&[u32], &str) = (&[0o400, 0o600], "token file must have mode 0400 or 0600");
#[cfg(not(target_os = "linux"))]
const TOKEN_MODES: (&[u32], &str) = (&[0o600], "token file must have mode 0600");

/// Ownership rule for the token file. Production requires root ownership.
/// Development with `--dev-same-user` requires ownership by the daemon euid.
pub(crate) fn check_token_metadata(
    mode: u32,
    owner_uid: u32,
    euid: u32,
    dev_same_user: bool,
) -> Result<()> {
    let (allowed, refusal) = TOKEN_MODES;
    if !allowed.contains(&(mode & 0o7777)) {
        bail!(refusal);
    }
    if dev_same_user {
        if owner_uid != euid {
            bail!("token file must be owned by the daemon user");
        }
    } else if owner_uid != 0 {
        bail!("token file must be owned by root");
    }
    Ok(())
}

/// Ownership rule for one trusted path. Root ownership always passes.
/// Development with `dev_same_user` also accepts ownership by the daemon euid,
/// so a user-owned bws under root-owned `/` and `/Users` still validates.
pub(crate) fn check_owner(owner_uid: u32, euid: u32, dev_same_user: bool) -> Result<()> {
    if owner_uid == 0 || (dev_same_user && owner_uid == euid) {
        return Ok(());
    }
    if dev_same_user {
        bail!("path must be owned by root or the daemon user");
    }
    bail!("path must be owned by root")
}

/// Writable bits rule for one trusted path. Group and other write bits fail.
pub(crate) fn check_not_group_or_world_writable(mode: u32) -> Result<()> {
    if mode & 0o022 != 0 {
        bail!("path must not be group writable or world writable");
    }
    Ok(())
}
