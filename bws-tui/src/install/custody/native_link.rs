//! Resolve a native path that a distro layout routes through symbolic links,
//! such as authselect's etc/pam.d/system-auth or a merged usr/sbin -> bin.
//! Each link is read by descriptor from a checked directory and must be owned
//! by root or the install owner. The kernel's own resolution must agree, and
//! the target must stay inside the install root. Callers still check the
//! target itself with the usual no-follow file rules.

use super::{Dir, Owner};
use crate::install::facts::{io_fact, refused};
use crate::install::sys::{c_path, lstat_at};
use crate::install::Fact;
use std::ffi::OsString;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};

/// Linux's own symlink hop limit is 40. Native layouts need one or two.
const MAX_LINKS: usize = 8;

impl Dir {
    /// The canonical logical path of `logical`, starting with `/`.
    pub(crate) fn native_target(&self, logical: &Path, owner: Owner) -> Fact<PathBuf> {
        let shown = logical.display();
        let mut pending = Vec::new();
        if let Err(why) = push_components(&mut pending, logical) {
            return Fact::Refused(format!("{shown} {why}"));
        }
        let mut resolved = PathBuf::new();
        let mut links = 0;
        while let Some(name) = pending.pop() {
            if name == ".." {
                if !resolved.pop() {
                    return Fact::Refused(format!("{shown} resolves outside the install root"));
                }
                continue;
            }
            let dir = match self.walk(&resolved, owner) {
                Fact::Known(super::Walk::Found(dir)) => dir,
                Fact::Known(super::Walk::Missing { .. }) => {
                    return Fact::Unknown(format!("{shown} changed while it was resolved"))
                }
                Fact::Refused(why) => return Fact::Refused(why),
                Fact::Unknown(why) => return Fact::Unknown(why),
            };
            let path = dir.path().join(&name);
            let stat = match lstat_at(dir.fd(), &name) {
                Ok(stat) => stat,
                Err(error) => return io_fact(&path, &error),
            };
            if stat.st_mode & libc::S_IFMT != libc::S_IFLNK {
                resolved.push(&name);
                continue;
            }
            links += 1;
            if links > MAX_LINKS {
                return refused(&path, "has too many levels of symbolic links".into());
            }
            if stat.st_uid != 0 && stat.st_uid != owner.0 {
                return refused(&path, format!("is a link owned by uid {}", stat.st_uid));
            }
            let target = match read_link(&dir, &name) {
                Ok(target) => target,
                Err(error) => return io_fact(&path, &error),
            };
            if target.is_absolute() {
                resolved = PathBuf::new();
            }
            if let Err(why) = push_components(&mut pending, &target) {
                return refused(&path, why);
            }
        }
        // The kernel must land on the same path. An absolute link in a
        // test root, for one, points outside it, so the two views disagree.
        let agree = self.path().canonicalize().and_then(|base| {
            let kernel = self
                .path()
                .join(logical.strip_prefix("/").unwrap_or(logical));
            Ok(kernel.canonicalize()? == base.join(&resolved))
        });
        match agree {
            Ok(true) => Fact::Known(Path::new("/").join(resolved)),
            Ok(false) => Fact::Refused(format!("{shown} resolves outside the install root")),
            Err(error) => io_fact(logical, &error),
        }
    }
}

/// Push a path's names onto the stack so the first name pops first.
fn push_components(pending: &mut Vec<String>, path: &Path) -> Result<(), String> {
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => names.push("..".to_owned()),
            Component::Normal(name) => match name.to_str() {
                Some(name) => names.push(name.to_owned()),
                None => return Err("has a link target that is not UTF-8".into()),
            },
            Component::Prefix(_) => return Err("has an unsupported link target".into()),
        }
    }
    pending.extend(names.into_iter().rev());
    Ok(())
}

fn read_link(dir: &Dir, name: &str) -> io::Result<PathBuf> {
    let raw = c_path(name.as_bytes())?;
    let mut buffer = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: the name is NUL-terminated and the buffer length is passed exactly.
    let length = unsafe {
        libc::readlinkat(
            dir.fd().as_raw_fd(),
            raw.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    let length = usize::try_from(length).map_err(|_| io::Error::last_os_error())?;
    if length == buffer.len() {
        return Err(io::Error::from_raw_os_error(libc::ENAMETOOLONG));
    }
    buffer.truncate(length);
    Ok(PathBuf::from(OsString::from_vec(buffer)))
}
