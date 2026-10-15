//! Private real directory trees for installer tests.

use super::layout::Layout;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A private anchor owned by the test user, removed on drop.
pub(crate) struct Tree {
    pub layout: Layout,
}

impl Tree {
    pub(crate) fn new(tag: &str) -> Self {
        let anchor = std::env::temp_dir().join(format!("hush-install-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&anchor);
        fs::create_dir_all(&anchor).unwrap();
        fs::set_permissions(&anchor, fs::Permissions::from_mode(0o700)).unwrap();
        // SAFETY: geteuid takes no arguments and cannot fail.
        let owner = unsafe { libc::geteuid() };
        Self {
            layout: Layout { anchor, owner },
        }
    }

    pub(crate) fn path(&self, logical: impl AsRef<Path>) -> PathBuf {
        self.layout.display(logical.as_ref())
    }

    /// Create every directory to `logical` at 0755, as a root-owned system tree would be.
    pub(crate) fn dirs(&self, logical: impl AsRef<Path>) -> PathBuf {
        let path = self.path(logical);
        fs::create_dir_all(&path).unwrap();
        let mut current = path.clone();
        while current != self.layout.anchor {
            fs::set_permissions(&current, fs::Permissions::from_mode(0o755)).unwrap();
            current.pop();
        }
        path
    }

    pub(crate) fn file(&self, logical: impl AsRef<Path>, bytes: &[u8], mode: u32) -> PathBuf {
        let logical = logical.as_ref();
        if let Some(parent) = logical.parent() {
            if !self.path(parent).exists() {
                self.dirs(parent);
            }
        }
        let path = self.path(logical);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    pub(crate) fn chmod(&self, logical: impl AsRef<Path>, mode: u32) {
        fs::set_permissions(self.path(logical), fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Every path and its mode and size, to prove a refusal wrote nothing.
    pub(crate) fn snapshot(&self) -> Vec<(PathBuf, u32, u64)> {
        let mut out = Vec::new();
        let mut stack = vec![self.layout.anchor.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let meta = fs::symlink_metadata(entry.path()).unwrap();
                if meta.is_dir() {
                    stack.push(entry.path());
                }
                out.push((entry.path(), meta.permissions().mode(), meta.len()));
            }
        }
        out.sort();
        out
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.layout.anchor);
    }
}
