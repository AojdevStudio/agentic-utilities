//! Exclusive staging and no-replace publication into a checked directory.
//! A file is written under a private staging name, given its final mode,
//! synced, then hard linked to its final name. The link fails when that name
//! exists, so nothing is ever replaced. A crash after the link leaves the
//! staging name as a second link that `finish_stage` removes on the next run.

use super::custody::{Dir, STAGE_PREFIX};
use super::sys::{c_path, check, create_exclusive, lstat_at, openat};
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn create_dir(parent: &Dir, name: &str, mode: u32) -> Result<()> {
    let path = parent.path().join(name);
    let raw = c_path(name.as_bytes())?;
    // SAFETY: the name is NUL-terminated and the parent descriptor is open.
    check(unsafe { libc::mkdirat(parent.fd().as_raw_fd(), raw.as_ptr(), 0o700) })
        .with_context(|| format!("cannot create {}", path.display()))?;
    let dir = openat(parent.fd(), name, libc::O_RDONLY | libc::O_DIRECTORY)
        .with_context(|| format!("cannot open the new {}", path.display()))?;
    // The directory is new and ours, so its exact mode only undoes the umask.
    // SAFETY: fchmod on an open descriptor this process owns.
    check(unsafe { libc::fchmod(dir.as_raw_fd(), mode as libc::mode_t) })
        .with_context(|| format!("cannot set the mode of {}", path.display()))?;
    sync(parent.fd()).with_context(|| format!("cannot sync {}", parent.path().display()))
}

pub(crate) fn publish(
    parent: &Dir,
    name: &str,
    mode: u32,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    let path = parent.path().join(name);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.subsec_nanos());
    let stage = format!("{STAGE_PREFIX}{name}.{}.{nanos}", std::process::id());
    let mut file = File::from(
        create_exclusive(parent.fd(), &stage)
            .with_context(|| format!("cannot stage {}", path.display()))?,
    );
    let written = write(&mut file).and_then(|()| {
        // SAFETY: fchmod on the staging file this process just created.
        check(unsafe { libc::fchmod(file.as_raw_fd(), mode as libc::mode_t) })?;
        Ok(file.sync_all()?)
    });
    let linked = written.and_then(|()| {
        let (raw_stage, raw_name) = (c_path(stage.as_bytes())?, c_path(name.as_bytes())?);
        let fd = parent.fd().as_raw_fd();
        // SAFETY: both names are NUL-terminated and resolve in the same open directory.
        check(unsafe { libc::linkat(fd, raw_stage.as_ptr(), fd, raw_name.as_ptr(), 0) })
            .with_context(|| format!("{} already exists and is left in place", path.display()))
    });
    unlink(parent.fd(), &stage)
        .with_context(|| format!("cannot remove the staging name for {}", path.display()))?;
    linked?;
    sync(parent.fd()).with_context(|| format!("cannot sync {}", parent.path().display()))
}

/// Remove this installer's leftover staging link once it is proven to be the
/// same complete file as the final name and its only other link.
pub(crate) fn finish_stage(parent: &Dir, name: &str, stage: &str) -> Result<()> {
    let path = parent.path().join(name);
    let ours = stage
        .strip_prefix(STAGE_PREFIX)
        .and_then(|rest| rest.strip_prefix(name))
        .is_some_and(|rest| rest.starts_with('.'));
    let same = ours
        && match (lstat_at(parent.fd(), name), lstat_at(parent.fd(), stage)) {
            (Ok(final_stat), Ok(stage_stat)) => {
                final_stat.st_dev == stage_stat.st_dev
                    && final_stat.st_ino == stage_stat.st_ino
                    && final_stat.st_nlink == 2
                    && final_stat.st_mode & libc::S_IFMT == libc::S_IFREG
            }
            _ => false,
        };
    if !same {
        bail!(
            "{} is not this installer's staging link for {}",
            stage,
            path.display()
        );
    }
    unlink(parent.fd(), stage)
        .with_context(|| format!("cannot remove the staging name for {}", path.display()))?;
    sync(parent.fd()).with_context(|| format!("cannot sync {}", parent.path().display()))
}

fn unlink(dir: BorrowedFd<'_>, name: &str) -> std::io::Result<()> {
    let raw = c_path(name.as_bytes())?;
    // SAFETY: the name is NUL-terminated and resolves in the open directory.
    check(unsafe { libc::unlinkat(dir.as_raw_fd(), raw.as_ptr(), 0) })
}

fn sync(dir: BorrowedFd<'_>) -> std::io::Result<()> {
    // SAFETY: fsync on an open directory descriptor.
    check(unsafe { libc::fsync(dir.as_raw_fd()) })
}
