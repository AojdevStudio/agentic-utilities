//! Threaded Unix socket listener with fixed directory and socket modes.

use super::daemon::{serve_connection_guarded, ServeConfig};
use anyhow::{bail, Context, Result};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::Path;

/// Socket directory mode. Any local user must traverse the directory to reach the socket.
/// The daemon owns the directory in production. Policy still uses kernel peer credentials.
pub(crate) const SOCKET_DIR_MODE: u32 = 0o755;

/// Socket mode. Any local user may connect so unprivileged clients reach the daemon.
/// Connection alone grants nothing. Policy uses kernel peer credentials on each connection.
pub(crate) const SOCKET_MODE: u32 = 0o666;

/// Whether an existing socket path has a listening daemon.
pub(crate) enum Liveness {
    /// Something answered or accepted the connect attempt.
    Live,
    /// Nothing is bound there; the path is safe to remove.
    Stale,
}

/// Deadline for the one liveness probe at daemon start.
const LIVENESS_PROBE_TIMEOUT_MS: i32 = 500;

/// Probe an existing socket path without blocking on a broken or backed-up
/// listener. `connect` runs nonblocking and completion is polled. A refusal
/// means the path is stale. Establishing a connection means it is live.
/// The probe cannot settle a connection that started but stayed pending, so
/// that case is unclear and the caller must not unlink anything.
pub(crate) fn socket_has_liveness(socket: &Path) -> Result<Liveness> {
    /// One probe result on the socket path.
    enum Probe {
        Live,
        Stale,
        Unclear(std::io::Error),
    }

    /// The probe connect runs nonblocking, so a dead or backed-up listener
    /// cannot stall the bind. Completion is polled within the deadline.
    fn probe(socket: &Path) -> Probe {
        // SAFETY: fd is closed on every path, and the sockaddr is built from
        // a zeroed struct plus checked path bytes. The path slice's only
        // cast is repr-identical bytes to i8 for the kernel struct.
        #[allow(unnecessary_transmutes)]
        unsafe {
            let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
            if fd < 0 {
                return Probe::Unclear(std::io::Error::last_os_error());
            }
            let flags = libc::fcntl(fd, libc::F_GETFL, 0);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                libc::close(fd);
                return Probe::Unclear(std::io::Error::last_os_error());
            }
            let mut addr: libc::sockaddr_un = std::mem::zeroed();
            addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
            let bytes = socket.as_os_str().as_bytes();
            if bytes.len() >= addr.sun_path.len() {
                libc::close(fd);
                return Probe::Unclear(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "socket path too long",
                ));
            }
            addr.sun_path[..bytes.len()]
                .copy_from_slice(std::mem::transmute::<&[u8], &[i8]>(bytes));
            let connected = libc::connect(
                fd,
                &addr as *const libc::sockaddr_un as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            );
            if connected == 0 {
                libc::close(fd);
                return Probe::Live;
            }
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                // Nothing bound, or it refused: the path is stale.
                Some(libc::ECONNREFUSED | libc::ENOENT | libc::ENOTSOCK) => {
                    libc::close(fd);
                    Probe::Stale
                }
                // A full pending queue still proves a listener exists.
                Some(libc::EAGAIN) => {
                    libc::close(fd);
                    Probe::Live
                }
                Some(libc::EINPROGRESS) => {
                    let mut pollfd = libc::pollfd {
                        fd,
                        events: libc::POLLHUP,
                        revents: 0,
                    };
                    let waited = libc::poll(&mut pollfd, 1, LIVENESS_PROBE_TIMEOUT_MS);
                    let outcome = if waited == 0 {
                        Probe::Live
                    } else if waited < 0 || pollfd.revents & libc::POLLERR != 0 {
                        Probe::Stale
                    } else {
                        Probe::Live
                    };
                    libc::close(fd);
                    outcome
                }
                _ => {
                    libc::close(fd);
                    Probe::Unclear(error)
                }
            }
        }
    }

    match probe(socket) {
        Probe::Live => Ok(Liveness::Live),
        Probe::Stale => Ok(Liveness::Stale),
        Probe::Unclear(error) => bail!(
            "socket liveness probe could not decide at {} ({}). The socket is left in place",
            socket.display(),
            error
        ),
    }
}

/// Bind `socket` with explicit directory and socket modes.
pub(crate) fn bind_listener(socket: &Path) -> Result<UnixListener> {
    if let Some(parent) = socket.parent() {
        if !parent.as_os_str().is_empty() {
            prepare_socket_dir(parent)?;
        }
    }
    // A live daemon answers; only a stale socket from a crashed daemon is removed.
    // The probe runs with a deadline, so a hung listener cannot stall the bind.
    match socket_has_liveness(socket) {
        Ok(Liveness::Live) => bail!(
            "another daemon is already listening at {}",
            socket.display()
        ),
        Ok(Liveness::Stale) => {}
        Err(error) => return Err(error),
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
            let metadata = std::fs::symlink_metadata(parent).with_context(|| {
                format!("failed to inspect socket directory at {}", parent.display())
            })?;
            if metadata.file_type().is_symlink() {
                bail!(
                    "socket directory must not be a symlink at {}",
                    parent.display()
                );
            }
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
