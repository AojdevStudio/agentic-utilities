//! Socket path resolution and trusted bws path checks.

use super::hardening::{check_not_group_or_world_writable, check_owner};
use anyhow::{bail, Context, Result};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

/// Absolute bws candidates checked when the daemon gets no `--bws-path`.
pub(crate) const DEFAULT_BWS_CANDIDATES: [&str; 2] = ["/usr/local/bin/bws", "/usr/bin/bws"];

/// Socket path. ADR 0002 fixes the default. `HUSH_SOCKET` overrides it.
pub(crate) fn default_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSH_SOCKET") {
        return PathBuf::from(path);
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/run/hush/hush.sock")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/run/hush/hush.sock")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        PathBuf::from("hush.sock")
    }
}

/// Default token location. The `daemon` subcommand can override it.
pub(crate) fn default_token_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/var/db/hush/token")
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from("/etc/hush/token")
    }
}

/// Resolve and validate the bws binary the root daemon will run.
/// The daemon never consults `PATH`.
pub(crate) fn resolve_trusted_bws(
    explicit: Option<PathBuf>,
    dev_same_user: bool,
) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return validate_trusted_bws_path(&path, dev_same_user);
    }
    let mut last_error: Option<anyhow::Error> = None;
    for candidate in DEFAULT_BWS_CANDIDATES {
        match validate_trusted_bws_path(Path::new(candidate), dev_same_user) {
            Ok(canonical) => return Ok(canonical),
            Err(error) => last_error = Some(error),
        }
    }
    let guidance = "no trusted bws binary found. Production expects a root owned \
copy at /usr/local/bin/bws with every path component root owned and not group \
writable or world writable";
    match last_error {
        Some(error) => Err(error).context(guidance),
        None => bail!(guidance),
    }
}

/// Validate one bws path and return its canonical form.
pub(crate) fn validate_trusted_bws_path(path: &Path, dev_same_user: bool) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("bws path must be absolute");
    }
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("bws binary missing at {}", path.display()))?;
    let metadata = std::fs::metadata(&canonical)
        .with_context(|| format!("bws binary missing at {}", canonical.display()))?;
    if !metadata.is_file() {
        bail!("bws path must be a regular file");
    }
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    let components = collect_path_components(&canonical)?;
    for component in &components {
        let meta = std::fs::metadata(component)
            .with_context(|| format!("bws path component missing at {}", component.display()))?;
        if component != &components[components.len() - 1] && !meta.is_dir() {
            bail!(
                "bws path component must be a directory at {}",
                component.display()
            );
        }
        check_owner(meta.uid(), euid, dev_same_user)
            .with_context(|| format!("bws path at {}", component.display()))?;
        check_not_group_or_world_writable(meta.mode())
            .with_context(|| format!("bws path at {}", component.display()))?;
    }
    Ok(canonical)
}

/// Build every prefix of an absolute canonical path from the root down.
fn collect_path_components(canonical: &Path) -> Result<Vec<PathBuf>> {
    let mut prefixes = Vec::new();
    let mut current = PathBuf::new();
    for component in canonical.components() {
        match component {
            Component::RootDir => {
                current.push("/");
                prefixes.push(current.clone());
            }
            Component::Normal(part) => {
                current.push(part);
                prefixes.push(current.clone());
            }
            Component::CurDir | Component::ParentDir => {
                bail!("bws canonical path contains a relative component");
            }
            Component::Prefix(_) => {
                bail!("bws canonical path contains a prefix component");
            }
        }
    }
    if prefixes.is_empty() {
        bail!("bws canonical path is empty");
    }
    Ok(prefixes)
}

/// Pure trusted path rule. Every owner and mode pair must pass the shared checks.
#[cfg(test)]
pub(crate) fn check_trusted_components(
    entries: &[(u32, u32)],
    euid: u32,
    dev_same_user: bool,
) -> Result<()> {
    for (index, (owner_uid, mode)) in entries.iter().enumerate() {
        check_owner(*owner_uid, euid, dev_same_user)
            .with_context(|| format!("bws path component {index}"))?;
        check_not_group_or_world_writable(*mode)
            .with_context(|| format!("bws path component {index}"))?;
    }
    Ok(())
}
