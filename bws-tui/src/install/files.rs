//! Checked descriptors for managed paths. A walk starts at the layout
//! anchor and opens one component at a time without following symlinks, so a
//! path string is never trusted as a capability.

use super::layout::{Kind, Layout, Spec};
use super::model::{Fact, Presence};
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Largest managed file install compares or accepts as operator policy.
const MAX_MANAGED_FILE: u64 = 64 * 1024;
/// Largest token source accepted by metadata. Its content is never read.
const MAX_TOKEN_SOURCE: i64 = 1024 * 1024;

/// Why a walk stopped before the object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    Missing,
    Denied,
    Unsafe(&'static str),
    Failed,
}

impl Stop {
    pub(crate) fn fact<T>(self) -> Fact<T> {
        match self {
            Self::Missing => Fact::Refused("a parent directory is missing"),
            Self::Denied => Fact::Unknown("needs root to inspect"),
            Self::Unsafe(reason) => Fact::Refused(reason),
            Self::Failed => Fact::Unknown("inspection failed"),
        }
    }
}

/// One opened object and the metadata read from that same descriptor.
pub(crate) struct Node {
    pub fd: OwnedFd,
    pub stat: libc::stat,
}

fn errno_stop() -> Stop {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ENOENT) => Stop::Missing,
        Some(libc::EACCES | libc::EPERM) => Stop::Denied,
        Some(libc::ELOOP | libc::EMLINK) => Stop::Unsafe("the path has a symlink"),
        Some(libc::ENOTDIR) => Stop::Unsafe("the path has a parent that is not a directory"),
        Some(libc::ENXIO | libc::EOPNOTSUPP) => Stop::Unsafe("the path is a special file"),
        _ => Stop::Failed,
    }
}

fn open_at(dir: RawFd, name: &OsStr, flags: libc::c_int) -> Result<OwnedFd, Stop> {
    let name = CString::new(name.as_bytes()).map_err(|_| Stop::Unsafe("the path has a NUL"))?;
    let flags = flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NOCTTY | libc::O_NONBLOCK;
    // SAFETY: openat reads one valid C string relative to an open directory.
    let fd = unsafe { libc::openat(dir, name.as_ptr(), flags) };
    if fd < 0 {
        return Err(errno_stop());
    }
    // SAFETY: openat returned a new descriptor that nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

pub(crate) fn fstat(fd: RawFd) -> Result<libc::stat, Stop> {
    // SAFETY: zeroed stat is a valid out buffer for fstat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat writes one stat for an open descriptor.
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return Err(Stop::Failed);
    }
    Ok(stat)
}

fn is_dir(stat: &libc::stat) -> bool {
    stat.st_mode & libc::S_IFMT == libc::S_IFDIR
}

fn is_file(stat: &libc::stat) -> bool {
    stat.st_mode & libc::S_IFMT == libc::S_IFREG
}

/// A directory on the way to a protected object. Only its owner may change it.
fn check_ancestor(layout: &Layout, fd: RawFd, stat: &libc::stat) -> Result<(), Stop> {
    if !is_dir(stat) {
        return Err(Stop::Unsafe("the path has a parent that is not a directory"));
    }
    if stat.st_uid != layout.owner {
        return Err(Stop::Unsafe("a parent directory is not owned by root"));
    }
    if (stat.st_mode as u32) & 0o022 != 0 {
        return Err(Stop::Unsafe("a parent directory is writable by others"));
    }
    if super::acl::grants_access(fd)? {
        return Err(Stop::Unsafe("a parent directory has an access list grant"));
    }
    Ok(())
}

/// Open the directory that holds `logical`, checking every directory from the
/// anchor down. Returns that directory and the final name.
pub(crate) fn open_parent(layout: &Layout, logical: &Path) -> Result<(OwnedFd, CString), Stop> {
    let mut parts = layout.components(logical);
    let leaf = parts.pop().ok_or(Stop::Unsafe("the path has no name"))?;
    let anchor = CString::new(layout.anchor.as_os_str().as_bytes())
        .map_err(|_| Stop::Unsafe("the path has a NUL"))?;
    // SAFETY: open reads one valid C string.
    let raw = unsafe { libc::open(anchor.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(errno_stop());
    }
    // SAFETY: open returned a new descriptor that nothing else owns.
    let mut dir = unsafe { OwnedFd::from_raw_fd(raw) };
    check_ancestor(layout, dir.as_raw_fd(), &fstat(dir.as_raw_fd())?)?;
    for part in &parts {
        dir = open_at(dir.as_raw_fd(), part, libc::O_RDONLY | libc::O_DIRECTORY)?;
        check_ancestor(layout, dir.as_raw_fd(), &fstat(dir.as_raw_fd())?)?;
    }
    let leaf = CString::new(leaf.as_bytes()).map_err(|_| Stop::Unsafe("the path has a NUL"))?;
    Ok((dir, leaf))
}

/// Open one object read-only after checking every directory above it.
pub(crate) fn open_node(layout: &Layout, logical: &Path, dir: bool) -> Result<Node, Stop> {
    let (parent, leaf) = open_parent(layout, logical)?;
    let flags = if dir {
        libc::O_RDONLY | libc::O_DIRECTORY
    } else {
        libc::O_RDONLY
    };
    let fd = open_at(parent.as_raw_fd(), OsStr::from_bytes(leaf.as_bytes()), flags)?;
    let stat = fstat(fd.as_raw_fd())?;
    Ok(Node { fd, stat })
}

/// Owner, type, exact mode, single link, and no access list grant.
fn check_leaf(layout: &Layout, node: &Node, mode: u32, dir: bool) -> Result<(), &'static str> {
    let stat = &node.stat;
    if dir && !is_dir(stat) {
        return Err("is not a directory");
    }
    if !dir && !is_file(stat) {
        return Err("is not a regular file");
    }
    if stat.st_uid != layout.owner {
        return Err("is not owned by root");
    }
    if (stat.st_mode as u32) & 0o7777 != mode {
        return Err("has unexpected permissions");
    }
    if !dir && stat.st_nlink != 1 {
        return Err("has another hard link");
    }
    match super::acl::grants_access(node.fd.as_raw_fd()) {
        Ok(false) => Ok(()),
        Ok(true) => Err("has an access list grant"),
        Err(_) => Err("has an access list that could not be read"),
    }
}

/// Read at most the managed bound, so a huge file cannot exhaust memory.
pub(crate) fn read_bounded(node: Node) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::from(node.fd)
        .take(MAX_MANAGED_FILE + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_MANAGED_FILE).then_some(bytes)
}

/// Inspect one managed artifact. A missing artifact whose parent is managed
/// too is absent, because the plan creates that parent first.
pub(crate) fn inspect(layout: &Layout, spec: &Spec, parent_managed: bool) -> Fact<Presence> {
    let dir = spec.kind == Kind::Dir;
    let node = match open_node(layout, &spec.path, dir) {
        Ok(node) => node,
        Err(Stop::Missing) => {
            return match open_parent(layout, &spec.path) {
                Ok(_) => Fact::Known(Presence::Absent),
                Err(Stop::Missing) if parent_managed => Fact::Known(Presence::Absent),
                Err(stop) => stop.fact(),
            }
        }
        Err(stop) => return stop.fact(),
    };
    let checked = check_leaf(layout, &node, spec.kind.mode(), dir);
    match &spec.kind {
        Kind::Dir => checked.map_or_else(Fact::Refused, |()| Fact::Known(Presence::Ready)),
        Kind::Token(_) => checked
            .and_then(|()| match node.stat.st_size as i64 {
                1..=MAX_TOKEN_SOURCE => Ok(()),
                _ => Err("has an empty or oversized size"),
            })
            .map_or_else(Fact::Refused, |()| Fact::Known(Presence::Ready)),
        Kind::File(want) => match (checked, read_bounded(node)) {
            (Err(reason), _) => Fact::Refused(reason),
            (Ok(()), Some(bytes)) if &bytes == want => Fact::Known(Presence::Ready),
            (Ok(()), _) => Fact::Refused("differs from what install writes"),
        },
        Kind::Pam(want) => match (checked, read_bounded(node)) {
            (Err(reason), _) => Fact::Refused(reason),
            (Ok(()), Some(bytes)) if &bytes == want => Fact::Known(Presence::Ready),
            (Ok(()), Some(bytes)) if !bytes.is_empty() && std::str::from_utf8(&bytes).is_ok() => {
                Fact::Known(Presence::Operator)
            }
            (Ok(()), _) => Fact::Refused("is empty, oversized, or not text"),
        },
    }
}

/// A root-owned executable that only root can change. Special bits and
/// extra links are refused, because either widens who controls the bytes.
pub(crate) fn check_executable(layout: &Layout, logical: &Path) -> Fact<()> {
    let node = match open_node(layout, logical, false) {
        Ok(node) => node,
        Err(Stop::Missing) => return Fact::Refused("the executable is missing"),
        Err(stop) => return stop.fact(),
    };
    let mode = node.stat.st_mode as u32;
    let rule = if !is_file(&node.stat) {
        Err("the executable is not a regular file")
    } else if node.stat.st_uid != layout.owner {
        Err("the executable is not owned by root")
    } else if mode & 0o7022 != 0 {
        Err("the executable is writable by others or has special bits")
    } else if mode & 0o100 == 0 {
        Err("the file is not executable")
    } else if node.stat.st_nlink != 1 {
        Err("the executable has another hard link")
    } else if super::acl::grants_access(node.fd.as_raw_fd()) != Ok(false) {
        Err("the executable has an access list grant")
    } else {
        Ok(())
    };
    rule.map_or_else(Fact::Refused, Fact::Known)
}

/// The persistent filesystem check for the systemd host key directory.
#[cfg(target_os = "linux")]
pub(crate) fn is_volatile(fd: RawFd) -> Option<bool> {
    // SAFETY: zeroed statfs is a valid out buffer for fstatfs.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: fstatfs writes one statfs for an open descriptor.
    if unsafe { libc::fstatfs(fd, &mut stat) } != 0 {
        return None;
    }
    let kind = stat.f_type as i64;
    // ramfs has no libc constant. Its magic is 0x858458f6.
    Some(kind == libc::TMPFS_MAGIC as i64 || kind == 0x8584_58f6)
}
