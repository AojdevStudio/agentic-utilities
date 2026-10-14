//! Daemon run policy from `/etc/hush/hushd.toml`.
//!
//! ```toml
//! # Runs that bind any of these keys use the dedicated hushrun account.
//! require_hushrun = ["PROD_DB_PASSWORD"]
//! ```

use super::hardening::{check_not_group_or_world_writable, check_owner};
use super::runspec::RunAs;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(crate) fn default_policy_path() -> PathBuf {
    PathBuf::from("/etc/hush/hushd.toml")
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    #[serde(default)]
    pub require_hushrun: Vec<String>,
}

impl Policy {
    /// A missing file is the empty policy. An existing file a caller could edit is refused.
    pub(crate) fn load(path: &Path, dev_same_user: bool) -> Result<Self> {
        // One descriptor is checked and read, so the file cannot be swapped in between.
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
        {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            other => other
                .with_context(|| format!("failed to open policy file at {}", path.display()))?,
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            bail!("policy file must be a regular file at {}", path.display());
        }
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        check_owner(metadata.uid(), euid, dev_same_user)
            .and_then(|()| check_not_group_or_world_writable(metadata.mode()))
            .with_context(|| format!("policy file at {}", path.display()))?;
        let mut text = String::new();
        file.read_to_string(&mut text)
            .with_context(|| format!("failed to read policy file at {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid policy file at {}", path.display()))
    }

    pub(crate) fn run_as_for<'a>(&self, keys: impl IntoIterator<Item = &'a str>) -> RunAs {
        let mut keys = keys.into_iter();
        if keys.any(|key| self.require_hushrun.iter().any(|wanted| wanted == key)) {
            RunAs::HushRun
        } else {
            RunAs::Requester
        }
    }
}
