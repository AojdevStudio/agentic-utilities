//! Bounded socket reachability with the connected peer's kernel identity.
//! A live socket alone proves nothing. Callers compare the peer with the
//! managed daemon. Anything this probe cannot settle is Unknown.

use super::facts::io_fact;
use super::Fact;
use crate::broker::peercred::{read_peer_credentials, PeerCredentials};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::Path;

const CONNECT_TIMEOUT_MS: libc::c_int = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// No socket, or a stale one that refuses connections.
    Nothing,
    /// A connection completed. These are the listener's credentials.
    Peer(PeerCredentials),
}

pub(crate) fn probe(socket: &Path) -> Fact<Reach> {
    match std::fs::symlink_metadata(socket) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Fact::Known(Reach::Nothing)
        }
        Err(error) => return io_fact(socket, &error),
        Ok(metadata) if !metadata.file_type().is_socket() => {
            return Fact::Refused(format!("{} is not a socket", socket.display()))
        }
        Ok(_) => {}
    }
    match connect(socket) {
        Ok(stream) => match read_peer_credentials(&stream) {
            Ok(peer) => Fact::Known(Reach::Peer(peer)),
            Err(_) => Fact::Unknown(format!(
                "the peer at {} has no readable identity",
                socket.display()
            )),
        },
        Err(error) => match error.raw_os_error() {
            Some(libc::ENOENT) => Fact::Known(Reach::Nothing),
            // Linux refuses only when nothing listens. macOS also refuses when the
            // accept queue is full, so there it proves nothing.
            Some(libc::ECONNREFUSED) if cfg!(target_os = "linux") => Fact::Known(Reach::Nothing),
            Some(libc::EAGAIN | libc::ECONNREFUSED | libc::ETIMEDOUT) => Fact::Unknown(format!(
                "the listener at {} is busy or saturated",
                socket.display()
            )),
            _ => io_fact(socket, &error),
        },
    }
}

/// Nonblocking connect, so a backed-up listener cannot stall status.
fn connect(socket: &Path) -> io::Result<UnixStream> {
    // SAFETY: socket returns a fresh descriptor or -1. The address is built
    // from a zeroed struct and length-checked path bytes.
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let owned = OwnedFd::from_raw_fd(fd);
        if libc::fcntl(
            fd,
            libc::F_SETFL,
            libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
        ) < 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut addr: libc::sockaddr_un = std::mem::zeroed();
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let bytes = socket.as_os_str().as_bytes();
        if bytes.len() >= addr.sun_path.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "socket path too long",
            ));
        }
        for (slot, byte) in addr.sun_path.iter_mut().zip(bytes) {
            *slot = *byte as libc::c_char;
        }
        let len = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        if libc::connect(fd, (&addr as *const libc::sockaddr_un).cast(), len) != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(error);
            }
            let mut pollfd = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            match libc::poll(&mut pollfd, 1, CONNECT_TIMEOUT_MS) {
                0 => return Err(io::Error::from_raw_os_error(libc::ETIMEDOUT)),
                ready if ready < 0 => return Err(io::Error::last_os_error()),
                _ => {}
            }
            let mut status: libc::c_int = 0;
            let mut size = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
            let rc = libc::getsockopt(
                owned.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&mut status as *mut libc::c_int).cast(),
                &mut size,
            );
            if rc != 0 {
                return Err(io::Error::last_os_error());
            }
            if status != 0 {
                return Err(io::Error::from_raw_os_error(status));
            }
        }
        Ok(UnixStream::from(owned))
    }
}

/// The listener is the managed daemon: the manager reports it active, the
/// connected peer is that main pid, and the peer runs as the expected owner.
pub(crate) fn is_managed(
    service: &super::facts::Service,
    peer: &PeerCredentials,
    owner: super::custody::Owner,
) -> bool {
    service.active && service.pid == Some(peer.pid) && peer.uid == owner.0
}
