//! Real filesystem trees laid out like a fresh Linux or macOS host, owned by
//! the test user, plus a snapshot that proves when nothing was written.

use super::custody::Owner;
use super::layout::{Layout, Platform};
use crate::broker::private_temp_harness::PrivateTemp;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) struct Fixture {
    pub temp: PrivateTemp,
    pub platform: Platform,
}

pub(crate) fn owner() -> Owner {
    // SAFETY: geteuid takes no arguments and cannot fail.
    Owner(unsafe { libc::geteuid() })
}

impl Fixture {
    pub(crate) fn new(platform: Platform) -> Self {
        let fixture = Self {
            temp: PrivateTemp::new("hush-install-test"),
            platform,
        };
        let (dirs, files): (&[&str], &[(&str, u32)]) = match platform {
            Platform::Linux => (
                &[
                    "etc",
                    "etc/systemd",
                    "etc/systemd/system",
                    "etc/pam.d",
                    "var",
                    "var/log",
                    "var/lib",
                    "var/lib/systemd",
                    "usr",
                    "usr/local",
                    "usr/local/bin",
                    "usr/sbin",
                    "run",
                ],
                &[
                    ("usr/local/bin/hush", 0o755),
                    ("usr/local/bin/bws", 0o755),
                    ("usr/sbin/nologin", 0o755),
                    ("etc/pam.d/common-auth", 0o644),
                    ("etc/pam.d/common-account", 0o644),
                ],
            ),
            Platform::MacOs => (
                &[
                    "Library",
                    "Library/LaunchDaemons",
                    "private",
                    "private/var",
                    "private/var/db",
                    "private/var/run",
                    "private/etc",
                    "usr",
                    "usr/bin",
                    "usr/local",
                    "usr/local/bin",
                ],
                &[
                    ("usr/local/bin/hush", 0o755),
                    ("usr/local/bin/bws", 0o755),
                    ("usr/bin/false", 0o755),
                ],
            ),
        };
        for dir in dirs {
            fixture.mkdir(dir, 0o755);
        }
        for (file, mode) in files {
            fixture.put(file, b"fixture\n", *mode);
        }
        if platform == Platform::MacOs {
            symlink("private/var", fixture.path("var")).unwrap();
            symlink("private/etc", fixture.path("etc")).unwrap();
        }
        fixture
    }

    pub(crate) fn layout(&self) -> Layout {
        Layout::in_root(self.temp.path(), owner(), self.platform)
    }

    pub(crate) fn path(&self, rel: &str) -> PathBuf {
        self.temp.path().join(rel.trim_start_matches('/'))
    }

    pub(crate) fn mkdir(&self, rel: &str, mode: u32) {
        fs::create_dir(self.path(rel)).unwrap();
        fs::set_permissions(self.path(rel), fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Change an existing directory's mode.
    pub(crate) fn mkdir_mode(&self, rel: &str, mode: u32) {
        fs::set_permissions(self.path(rel), fs::Permissions::from_mode(mode)).unwrap();
    }

    pub(crate) fn put(&self, rel: &str, bytes: &[u8], mode: u32) {
        fs::write(self.path(rel), bytes).unwrap();
        fs::set_permissions(self.path(rel), fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Every entry with its mode, link count and bytes or link target.
    pub(crate) fn snapshot(&self) -> BTreeMap<PathBuf, (u32, u64, Vec<u8>)> {
        let mut entries = BTreeMap::new();
        walk(self.temp.path(), self.temp.path(), &mut entries);
        entries
    }
}

fn walk(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, (u32, u64, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let bytes = if metadata.file_type().is_symlink() {
            fs::read_link(&path)
                .unwrap()
                .into_os_string()
                .into_encoded_bytes()
        } else if metadata.is_file() {
            fs::read(&path).unwrap()
        } else {
            Vec::new()
        };
        entries.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            (metadata.mode(), metadata.nlink(), bytes),
        );
        if metadata.is_dir() {
            walk(root, &path, entries);
        }
    }
}
