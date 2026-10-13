//! Pure ownership and mode checks shared by the token file and the bws binary.

use anyhow::{bail, Result};

/// Ownership rule for the token file. Production requires root ownership.
/// Development with `--dev-same-user` requires ownership by the daemon euid.
pub(crate) fn check_token_metadata(
    mode: u32,
    owner_uid: u32,
    euid: u32,
    dev_same_user: bool,
) -> Result<()> {
    if mode & 0o777 != 0o600 {
        bail!("token file must have mode 0600");
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

/// Ownership rule for one trusted path. Production requires root ownership.
/// Development with `dev_same_user` requires ownership by the daemon euid.
pub(crate) fn check_owner(owner_uid: u32, euid: u32, dev_same_user: bool) -> Result<()> {
    if dev_same_user {
        if owner_uid != euid {
            bail!("path must be owned by the daemon user");
        }
    } else if owner_uid != 0 {
        bail!("path must be owned by root");
    }
    Ok(())
}

/// Writable bits rule for one trusted path. Group and other write bits fail.
pub(crate) fn check_not_group_or_world_writable(mode: u32) -> Result<()> {
    if mode & 0o022 != 0 {
        bail!("path must not be group writable or world writable");
    }
    Ok(())
}
