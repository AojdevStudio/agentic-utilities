//! Socket path resolution. ADR 0002 fixes the default. `HUSH_SOCKET` overrides it.

use std::path::PathBuf;

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

/// Default token location. The `daemon` subcommand can override it for tests.
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

/// First candidate on `PATH` that exists, or an error naming no secrets.
pub(crate) fn resolve_on_path(name: &str) -> anyhow::Result<PathBuf> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("{name} not found on PATH")
}
