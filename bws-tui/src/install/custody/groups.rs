//! Ubuntu ships /var/log as root:syslog 0775 so its logging daemon can write
//! there. That one directory, the log dir's parent, may carry the group write
//! bit of a named system group. The install root's own etc/group must pass the
//! protected file checks and bind the name to one gid that no other line uses.
//! Root controls that file, so the members it lists are trusted. Those members
//! can create, rename, or remove entries in /var/log, including /var/log/hush
//! itself. The trust is deliberately limited to the log dir's parent, and only
//! on the walk to the log dir itself. A walk to any other path through
//! /var/log, such as a program or a file inside the log dir, trusts nothing,
//! because those members could swap what it finds. Any doubt trusts nothing, so
//! the usual refusal stands. macOS has no log dir here and gains nothing.

use super::{regular_file, Dir, Owner, Walk};
use crate::install::layout::{rel, Layout};
use crate::install::sys::openat;
use crate::install::Fact;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const SYSTEM_GROUPS: [&str; 2] = ["syslog", "adm"];
/// Far above any local group file. A longer one is not parsed at all.
const LIMIT: usize = 1 << 20;

/// The log dir, whose parent alone may be group writable, and the gids it may carry.
#[derive(Debug, Clone, Default)]
pub(super) struct Shared {
    log_dir: Option<PathBuf>,
    gids: Vec<u32>,
}

impl Shared {
    pub(super) fn allows(&self, path: &Path, gid: u32) -> bool {
        self.log_dir.as_deref().and_then(Path::parent) == Some(path) && self.gids.contains(&gid)
    }

    /// This trust for a walk that ends at the log dir, and none for any other walk.
    pub(super) fn toward(&self, target: &Path) -> Shared {
        match self.log_dir.as_deref() == Some(target) {
            true => self.clone(),
            false => Shared::default(),
        }
    }
}

impl Dir {
    /// This install root, letting the log dir's parent carry a vouched system group's write bit.
    pub(crate) fn trusting_log_parent(mut self, layout: &Layout) -> Dir {
        if let (Some(dir), Some(text)) = (layout.log_dir(), self.group_file(layout)) {
            self.shared = Shared {
                log_dir: Some(self.path.join(rel(Path::new(dir)))),
                gids: vouched(&text),
            };
        }
        self
    }

    /// Walk relative components below this directory, checking each one.
    /// No directory on the way may be group writable.
    pub(crate) fn walk(&self, rel: &Path, owner: Owner) -> Fact<Walk> {
        self.walk_with(rel, owner, Shared::default())
    }

    /// Walk to one of the installer's own directories. Only the walk to the
    /// log dir itself keeps the trust in its shared parent.
    pub(crate) fn walk_own_dir(&self, rel: &Path, owner: Owner) -> Fact<Walk> {
        self.walk_with(rel, owner, self.shared.toward(&self.path.join(rel)))
    }

    fn group_file(&self, layout: &Layout) -> Option<String> {
        let Fact::Known(Walk::Found(etc)) = self.walk(Path::new("etc"), layout.owner) else {
            return None;
        };
        let fd = openat(etc.fd(), "group", libc::O_RDONLY | libc::O_NONBLOCK).ok()?;
        let stat = regular_file::<()>(&fd, &etc.path.join("group"), layout.owner).ok()?;
        if stat.st_mode as u32 & 0o7022 != 0 || stat.st_nlink != 1 {
            return None;
        }
        let mut text = String::new();
        File::from(fd)
            .take(LIMIT as u64 + 1)
            .read_to_string(&mut text)
            .ok()?;
        (text.len() <= LIMIT).then_some(text)
    }
}

/// Gids of the system groups named on exactly one well-formed line, with a gid no other line uses.
fn vouched(text: &str) -> Vec<u32> {
    let lines: Vec<Vec<&str>> = text.lines().map(|line| line.split(':').collect()).collect();
    let gid_of = |fields: &[&str]| fields.get(2).and_then(|gid| gid.parse::<u32>().ok());
    SYSTEM_GROUPS
        .iter()
        .filter_map(|name| {
            let mut named = lines.iter().filter(|fields| fields.first() == Some(name));
            let (fields, None) = (named.next()?, named.next()) else {
                return None;
            };
            let [_, _, _, _] = fields.as_slice() else {
                return None;
            };
            let gid = gid_of(fields)?;
            let users = lines.iter().filter(|fields| gid_of(fields) == Some(gid));
            (users.count() == 1).then_some(gid)
        })
        .collect()
}
