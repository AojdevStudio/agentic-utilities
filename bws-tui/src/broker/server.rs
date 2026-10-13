//! Threaded Unix socket listener with fixed directory and socket modes.

use super::daemon::{serve_connection_guarded, ServeConfig};
use anyhow::{bail, Context, Result};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::Path;

/// Socket directory mode. Any local user must traverse the directory to reach the socket.
/// The daemon owns the directory in production. Policy still uses kernel peer credentials.
pub(crate) const SOCKET_DIR_MODE: u32 = 0o755;

/// Socket mode. Any local user may connect so unprivileged clients reach the daemon.
/// Connection alone grants nothing. Policy uses kernel peer credentials on each connection.
pub(crate) const SOCKET_MODE: u32 = 0o666;

/// Bind `socket` with explicit directory and socket modes.
pub(crate) fn bind_listener(socket: &Path) -> Result<UnixListener> {
    if let Some(parent) = socket.parent() {
        if !parent.as_os_str().is_empty() {
            prepare_socket_dir(parent)?;
        }
    }
    if let Ok(metadata) = std::fs::symlink_metadata(socket) {
        if !metadata.file_type().is_socket() {
            bail!("refusing to remove non-socket path at {}", socket.display());
        }
        std::fs::remove_file(socket)
            .with_context(|| format!("failed to remove stale socket at {}", socket.display()))?;
    }
    let listener = UnixListener::bind(socket)
        .with_context(|| format!("failed to bind daemon socket at {}", socket.display()))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(SOCKET_MODE))
        .with_context(|| format!("failed to set socket mode at {}", socket.display()))?;
    Ok(listener)
}

/// Accept connections forever. Each connection runs on its own thread.
pub(crate) fn run_server(
    listener: UnixListener,
    backend: std::sync::Arc<dyn super::backend::VaultBackend>,
    audit: std::sync::Arc<dyn super::audit::AuditLog>,
    config: ServeConfig,
) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let active = std::sync::Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("accept failed: {error}");
                continue;
            }
        };
        if active.load(Ordering::SeqCst) >= config.max_connections {
            drop(stream);
            continue;
        }
        active.fetch_add(1, Ordering::SeqCst);
        let backend = std::sync::Arc::clone(&backend);
        let audit = std::sync::Arc::clone(&audit);
        let active = std::sync::Arc::clone(&active);
        std::thread::spawn(move || {
            let _slot = ConnectionSlot(active);
            if let Err(error) = serve_connection_guarded(stream, &*backend, &*audit, config) {
                eprintln!("connection failed: {error}");
            }
        });
    }
    Ok(())
}

/// Create or validate the socket directory.
pub(crate) fn prepare_socket_dir(parent: &Path) -> Result<()> {
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    prepare_socket_dir_as(parent, euid)
}

/// Create or validate the socket directory for the daemon euid.
fn prepare_socket_dir_as(parent: &Path, euid: u32) -> Result<()> {
    match std::fs::create_dir(parent) {
        Ok(()) => {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(SOCKET_DIR_MODE))
                .with_context(|| {
                    format!(
                        "failed to set socket directory mode at {}",
                        parent.display()
                    )
                })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::metadata(parent).with_context(|| {
                format!("failed to inspect socket directory at {}", parent.display())
            })?;
            if !metadata.is_dir() {
                bail!(
                    "socket parent path must be a directory at {}",
                    parent.display()
                );
            }
            check_socket_dir(metadata.uid(), metadata.mode(), euid)
                .with_context(|| format!("socket directory at {}", parent.display()))
        }
        Err(error) => match parent.parent() {
            Some(ancestor) if !ancestor.as_os_str().is_empty() && !ancestor.is_dir() => {
                prepare_socket_dir_as(ancestor, euid)?;
                std::fs::create_dir(parent).with_context(|| {
                    format!("failed to create socket directory at {}", parent.display())
                })?;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(SOCKET_DIR_MODE))
                    .with_context(|| {
                        format!(
                            "failed to set socket directory mode at {}",
                            parent.display()
                        )
                    })
            }
            _ => Err(error).with_context(|| {
                format!("failed to create socket directory at {}", parent.display())
            }),
        },
    }
}

/// Pure rule for an existing socket directory.
pub(crate) fn check_socket_dir(owner_uid: u32, mode: u32, euid: u32) -> Result<()> {
    if owner_uid != euid {
        bail!("socket directory must be owned by the daemon user");
    }
    if mode & 0o022 != 0 {
        bail!("socket directory must not be group writable or world writable");
    }
    Ok(())
}

/// Releases one connection slot when the worker thread ends.
struct ConnectionSlot(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}
