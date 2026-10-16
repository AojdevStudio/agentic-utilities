//! Select and verify the canonical native executable before the runner spawns it.

use super::custody::{Dir, Owner};
use super::facts::Fact;
use super::layout::rel;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub(super) fn checked(root: &Dir, owner: Owner, program: &str) -> Result<PathBuf> {
    let path = match root.native_target(Path::new(program), owner) {
        Fact::Known(path) => path,
        Fact::Refused(why) | Fact::Unknown(why) => {
            bail!("{program} is not a trusted native program ({why})");
        }
    };
    if let Fact::Refused(why) | Fact::Unknown(why) = root.executable(rel(&path), owner) {
        bail!("{program} is not a trusted native program ({why})");
    }
    Ok(root.path().join(rel(&path)))
}
