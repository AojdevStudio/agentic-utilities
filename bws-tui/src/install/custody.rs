//! Checked descriptor walks for protected install paths. Each component is
//! opened with O_NOFOLLOW from an already checked directory, and every check
//! runs on that open descriptor, so a path cannot be swapped between check and use.

use super::facts::{io_fact, refused};
use super::sys::{c_path, fstat, openat, owned};
use super::{acl, Fact};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

/// Prefix of this installer's private staging names:
/// `<STAGE_PREFIX><final name>.<pid>.<nanos>`.
pub(crate) const STAGE_PREFIX: &str = ".hush-stage.";

/// Expected owner of protected paths. Root in production, the test user in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Owner(pub u32);

/// One open directory on a checked chain.
pub(crate) struct Dir {
    fd: OwnedFd,
    path: PathBuf,
    mode: u32,
}

pub(crate) enum Walk {
    Found(Dir),
    /// The first missing component's parent and every name still to create.
    Missing {
        parent: Dir,
        rest: Vec<String>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FileState {
    Absent,
    Ready,
    /// The file is complete, and its only other link is this installer's own staging name.
    Resume {
        stage: String,
    },
}

/// The mode rule for one file. Special bits are always refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Exactly these permission bits.
    Exactly(u32),
    /// Any mode with no group or other write, such as an operator's PAM file.
    Protected,
    /// Owner bits only, such as a key the service manager created.
    Private,
}

pub(crate) enum Contents<'a> {
    Opaque,
    Exactly(&'a [u8]),
}

impl Dir {
    pub(crate) fn open_root(path: &Path, owner: Owner) -> Fact<Dir> {
        let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = match c_path(path.as_os_str().as_bytes()) {
            // SAFETY: the path is NUL-terminated and open returns an owned descriptor or -1.
            Ok(raw) => owned(unsafe { libc::open(raw.as_ptr(), flags) }),
            Err(error) => Err(error),
        };
        match fd {
            Ok(fd) => Dir::checked(fd, path.to_path_buf(), owner),
            Err(error) => io_fact(path, &error),
        }
    }

    /// Walk relative components below this directory, checking each one.
    pub(crate) fn walk(&self, rel: &Path, owner: Owner) -> Fact<Walk> {
        let mut names = Vec::new();
        for component in rel.components() {
            let Component::Normal(name) = component else {
                return Fact::Refused(format!("{} is not a plain relative path", rel.display()));
            };
            names.push(name.to_string_lossy().into_owned());
        }
        let mut current = match self.fd.try_clone() {
            Ok(fd) => Dir {
                fd,
                path: self.path.clone(),
                mode: self.mode,
            },
            Err(error) => return io_fact(&self.path, &error),
        };
        for (index, name) in names.iter().enumerate() {
            let path = current.path.join(name);
            match openat(current.fd(), name, libc::O_RDONLY | libc::O_DIRECTORY) {
                Ok(fd) => match Dir::checked(fd, path, owner) {
                    Fact::Known(dir) => current = dir,
                    Fact::Refused(why) => return Fact::Refused(why),
                    Fact::Unknown(why) => return Fact::Unknown(why),
                },
                Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                    let rest = names[index..].to_vec();
                    return Fact::Known(Walk::Missing {
                        parent: current,
                        rest,
                    });
                }
                Err(error) => return io_fact(&path, &error),
            }
        }
        Fact::Known(Walk::Found(current))
    }

    /// Check one file in this directory against its exact mode and owner.
    pub(crate) fn file(
        &self,
        name: &str,
        mode: Mode,
        owner: Owner,
        contents: Contents<'_>,
    ) -> Fact<FileState> {
        let path = self.path.join(name);
        let fd = match openat(self.fd(), name, libc::O_RDONLY | libc::O_NONBLOCK) {
            Ok(fd) => fd,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                return Fact::Known(FileState::Absent)
            }
            Err(error) => return io_fact(&path, &error),
        };
        let stat = match regular_file(&fd, &path, owner) {
            Ok(stat) => stat,
            Err(fact) => return fact,
        };
        let actual = stat.st_mode as u32 & 0o7777;
        let allowed = match mode {
            Mode::Exactly(expected) => actual == expected,
            Mode::Protected => actual & 0o7022 == 0,
            Mode::Private => actual & 0o7077 == 0,
        };
        if !allowed {
            return refused(
                &path,
                format!("has mode {actual:o}, which hush install does not accept here"),
            );
        }
        let state = match stat.st_nlink {
            1 => FileState::Ready,
            2 => match self.own_stage(name, &stat) {
                Some(stage) => FileState::Resume { stage },
                None => return refused(&path, "has another hard link".into()),
            },
            _ => return refused(&path, "has other hard links".into()),
        };
        if let Contents::Exactly(expected) = contents {
            let mut actual = Vec::new();
            let read = File::from(fd)
                .take(expected.len() as u64 + 1)
                .read_to_end(&mut actual);
            match read {
                Ok(_) if actual == expected => {}
                Ok(_) => return refused(&path, "differs from what hush install writes".into()),
                Err(error) => return io_fact(&path, &error),
            }
        }
        Fact::Known(state)
    }

    /// Check a protected executable: regular, executable by its owner, with no
    /// special bits, no group or other write, one link and no ACL grants.
    pub(crate) fn executable(&self, rel: &Path, owner: Owner) -> Fact<()> {
        let (Some(parent), Some(name)) = (rel.parent(), rel.file_name()) else {
            return Fact::Refused(format!("{} names no file", rel.display()));
        };
        let dir = match self.walk(parent, owner) {
            Fact::Known(Walk::Found(dir)) => dir,
            Fact::Known(Walk::Missing { .. }) => {
                return Fact::Refused(format!("{} is missing", self.path.join(rel).display()))
            }
            Fact::Refused(why) => return Fact::Refused(why),
            Fact::Unknown(why) => return Fact::Unknown(why),
        };
        let path = dir.path.join(name);
        let name = name.to_string_lossy();
        let stat = match openat(dir.fd(), &name, libc::O_RDONLY | libc::O_NONBLOCK) {
            Ok(fd) => match regular_file(&fd, &path, owner) {
                Ok(stat) => stat,
                Err(fact) => return fact,
            },
            Err(error) => return io_fact(&path, &error),
        };
        let mode = stat.st_mode as u32;
        if mode & 0o7022 != 0 || mode & 0o100 == 0 {
            return refused(
                &path,
                format!(
                    "has mode {:o}, expected an executable with no special bits or shared write",
                    mode & 0o7777
                ),
            );
        }
        if stat.st_nlink != 1 {
            return refused(&path, "has other hard links".into());
        }
        Fact::Known(())
    }

    pub(crate) fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn mode(&self) -> u32 {
        self.mode
    }

    fn checked(fd: OwnedFd, path: PathBuf, owner: Owner) -> Fact<Dir> {
        let stat = match fstat(fd.as_fd()) {
            Ok(stat) => stat,
            Err(error) => return io_fact(&path, &error),
        };
        let mode = stat.st_mode as u32;
        if stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
            return refused(&path, "is not a directory".into());
        }
        if stat.st_uid != 0 && stat.st_uid != owner.0 {
            return refused(&path, format!("is owned by uid {}", stat.st_uid));
        }
        if mode & 0o022 != 0 {
            return refused(&path, "is group or world writable".into());
        }
        match acl::grants(fd.as_fd()) {
            Ok(false) => Fact::Known(Dir {
                fd,
                path,
                mode: mode & 0o7777,
            }),
            Ok(true) => refused(&path, "has extended ACL grants".into()),
            Err(error) => io_fact(&path, &error),
        }
    }

    /// The installer's own staging link to the same inode, left by a crash
    /// between publishing the final name and removing the staging name.
    fn own_stage(&self, name: &str, stat: &libc::stat) -> Option<String> {
        let prefix = format!("{STAGE_PREFIX}{name}.");
        std::fs::read_dir(&self.path)
            .ok()?
            .flatten()
            .find_map(|entry| {
                let candidate = entry.file_name().to_str()?.to_owned();
                let raw = c_path(
                    candidate
                        .strip_prefix(&prefix)
                        .map(|_| candidate.as_bytes())?,
                )
                .ok()?;
                // SAFETY: zeroed stat is a valid out buffer, and the name is NUL-terminated.
                let mut link: libc::stat = unsafe { std::mem::zeroed() };
                let rc = unsafe {
                    libc::fstatat(
                        self.fd.as_raw_fd(),
                        raw.as_ptr(),
                        &mut link,
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                };
                (rc == 0 && link.st_dev == stat.st_dev && link.st_ino == stat.st_ino)
                    .then_some(candidate)
            })
    }
}

fn regular_file<T>(fd: &OwnedFd, path: &Path, owner: Owner) -> Result<libc::stat, Fact<T>> {
    let stat = fstat(fd.as_fd()).map_err(|error| io_fact(path, &error))?;
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(refused(path, "is not a regular file".into()));
    }
    if stat.st_uid != owner.0 {
        return Err(refused(path, format!("is owned by uid {}", stat.st_uid)));
    }
    match acl::grants(fd.as_fd()) {
        Ok(false) => Ok(stat),
        Ok(true) => Err(refused(path, "has extended ACL grants".into())),
        Err(error) => Err(io_fact(path, &error)),
    }
}
