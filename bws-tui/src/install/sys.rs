//! Raw descriptor calls shared by custody checks and publication.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

pub(crate) fn openat(dir: BorrowedFd<'_>, name: &str, flags: libc::c_int) -> io::Result<OwnedFd> {
    let raw = c_path(name.as_bytes())?;
    let flags = flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NOCTTY;
    // SAFETY: the name is NUL-terminated and openat returns an owned descriptor or -1.
    owned(unsafe { libc::openat(dir.as_raw_fd(), raw.as_ptr(), flags) })
}

pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<libc::stat> {
    // SAFETY: zeroed stat is a valid out buffer for fstat on an open descriptor.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    match unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } {
        0 => Ok(stat),
        _ => Err(io::Error::last_os_error()),
    }
}

pub(crate) fn c_path(bytes: &[u8]) -> io::Result<CString> {
    CString::new(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

pub(crate) fn owned(fd: libc::c_int) -> io::Result<OwnedFd> {
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a nonnegative return is a fresh descriptor this process now owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Create `name` exclusively in `dir` at 0600, never following a symlink.
pub(crate) fn create_exclusive(dir: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
    let raw = c_path(name.as_bytes())?;
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: the name is NUL-terminated and the mode argument matches O_CREAT.
    owned(unsafe { libc::openat(dir.as_raw_fd(), raw.as_ptr(), flags, 0o600 as libc::c_uint) })
}

/// Run one `*at` call that returns 0 on success.
pub(crate) fn check(rc: libc::c_int) -> io::Result<()> {
    match rc {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

/// Stat `name` in `dir` without following a symlink.
pub(crate) fn lstat_at(dir: BorrowedFd<'_>, name: &str) -> io::Result<libc::stat> {
    let raw = c_path(name.as_bytes())?;
    // SAFETY: zeroed stat is a valid out buffer, and the name is NUL-terminated.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    check(unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            raw.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    })?;
    Ok(stat)
}
