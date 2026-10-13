//! Daemon token custody checks. The token value never enters an error.

use super::hardening::check_token_metadata;
use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::fs::File;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::FromRawFd;
use std::path::Path;

/// Read the token from one open file description that passed every check.
pub(crate) fn load_checked(path: &Path, dev_same_user: bool) -> Result<SecretString> {
    let mut file = open_nofollow(path)?;
    // SAFETY: fstat fills a zeroed stat buffer for a valid file description.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let fd = {
        use std::os::unix::io::AsRawFd;
        file.as_raw_fd()
    };
    // SAFETY: `fd` is an owned file description held by `file`. `stat` is writable.
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("failed to stat token file at {}", path.display()));
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        bail!("token file must be a regular file");
    }
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    check_token_metadata(u32::from(stat.st_mode), stat.st_uid, euid, dev_same_user)?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .with_context(|| format!("failed to read token file at {}", path.display()))?;
    let text = String::from_utf8(raw)?.trim().to_string();
    if text.is_empty() {
        bail!("token file is empty");
    }
    Ok(SecretString::from(text))
}

/// Open `path` without following a final symlink.
fn open_nofollow(path: &Path) -> Result<File> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&0) {
        bail!("token file path contains a NUL byte");
    }
    let cpath = std::ffi::CString::new(bytes)
        .with_context(|| format!("token file missing at {}", path.display()))?;
    // SAFETY: `cpath` is a NUL terminated path. Flags request read-only access.
    let fd = unsafe {
        libc::open(
            cpath.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("token file missing at {}", path.display()));
    }
    // SAFETY: `fd` is a new owned file description. It is not used after this call.
    Ok(unsafe { File::from_raw_fd(fd) })
}
