//! Publication into checked directories. A file is staged under a private
//! name, completed, synced, and moved to its final name only while that name
//! is free. Nothing existing is replaced, repaired, or removed.

use super::files::{inspect, open_parent, Stop};
use super::layout::{Layout, Spec};
use super::model::{Fact, Presence};
use anyhow::{anyhow, bail, Result};
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;

fn stop_error(stop: Stop) -> anyhow::Error {
    anyhow!(match stop {
        Stop::Missing => "a parent directory is missing",
        Stop::Denied => "permission denied",
        Stop::Unsafe(reason) => reason,
        Stop::Failed => "inspection failed",
    })
}

fn fsync_dir(dir: RawFd) -> Result<()> {
    // SAFETY: fsync takes one open descriptor.
    if unsafe { libc::fsync(dir) } != 0 {
        bail!("could not sync the directory ({})", io::Error::last_os_error());
    }
    Ok(())
}

/// Create one directory and set its exact mode, since the umask may narrow
/// it. Only a directory this call created is ever changed.
pub(crate) fn create_dir(layout: &Layout, logical: &Path, mode: u32) -> Result<()> {
    let (parent, leaf) = open_parent(layout, logical).map_err(stop_error)?;
    // SAFETY: mkdirat reads one valid C string relative to an open directory.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), leaf.as_ptr(), 0o700) } != 0 {
        bail!("could not create the directory ({})", io::Error::last_os_error());
    }
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: openat reads one valid C string relative to an open directory.
    let raw = unsafe { libc::openat(parent.as_raw_fd(), leaf.as_ptr(), flags) };
    if raw < 0 {
        bail!("could not open the new directory ({})", io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor that nothing else owns.
    let dir = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: fchmod changes the mode of the directory this call created.
    if unsafe { libc::fchmod(dir.as_raw_fd(), mode as libc::mode_t) } != 0 {
        bail!("could not set the directory mode ({})", io::Error::last_os_error());
    }
    fsync_dir(parent.as_raw_fd())
}

#[cfg(target_os = "linux")]
fn rename_noreplace(dir: RawFd, from: &CString, to: &CString) -> io::Result<()> {
    // SAFETY: both names are valid C strings relative to the same open directory.
    let rc = unsafe {
        libc::renameat2(dir, from.as_ptr(), dir, to.as_ptr(), libc::RENAME_NOREPLACE)
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn rename_noreplace(dir: RawFd, from: &CString, to: &CString) -> io::Result<()> {
    // SAFETY: both names are valid C strings relative to the same open directory.
    let rc = unsafe {
        libc::renameatx_np(dir, from.as_ptr(), dir, to.as_ptr(), libc::RENAME_EXCL)
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Stage, fill, and publish `spec` without replacing anything. A target that
/// appeared meanwhile counts as done only when it passes the same checks.
/// The rename moves the one staged link, so a crash leaves either a private
/// stage or the final file, never both names on one file.
pub(crate) fn publish(
    layout: &Layout,
    spec: &Spec,
    fill: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    let (parent, leaf) = open_parent(layout, &spec.path).map_err(stop_error)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let stage = CString::new(format!(
        ".{}.hush-stage-{}-{nanos}",
        leaf.to_string_lossy(),
        std::process::id()
    ))?;
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    // SAFETY: openat reads one valid C string relative to an open directory.
    let raw = unsafe { libc::openat(parent.as_raw_fd(), stage.as_ptr(), flags, 0o600) };
    if raw < 0 {
        bail!("could not create a staging file ({})", io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor that nothing else owns.
    let mut file = File::from(unsafe { OwnedFd::from_raw_fd(raw) });
    let staged = fill(&mut file).and_then(|()| {
        // SAFETY: fchmod changes the mode of the file this call created.
        if unsafe { libc::fchmod(file.as_raw_fd(), spec.kind.mode() as libc::mode_t) } != 0 {
            bail!("could not set the file mode ({})", io::Error::last_os_error());
        }
        file.sync_all()?;
        Ok(())
    });
    let published = staged.and_then(|()| {
        match rename_noreplace(parent.as_raw_fd(), &stage, &leaf) {
            Ok(()) => fsync_dir(parent.as_raw_fd()),
            Err(error) if error.raw_os_error() == Some(libc::EEXIST) => {
                match inspect(layout, spec, true) {
                    Fact::Known(Presence::Ready) => Ok(()),
                    _ => bail!("another file appeared at the target while install ran"),
                }
            }
            Err(error) => bail!("could not publish the file ({error})"),
        }
    });
    // SAFETY: unlinkat removes only the private stage this call created. After
    // a successful rename the name is gone and this is a harmless ENOENT.
    unsafe { libc::unlinkat(parent.as_raw_fd(), stage.as_ptr(), 0) };
    published
}
