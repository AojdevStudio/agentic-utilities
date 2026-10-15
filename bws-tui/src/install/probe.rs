//! Who answers on the broker socket. A live socket alone proves nothing. The
//! peer must be root and the manager's own service pid, on the same
//! connection this probe opened.

use super::model::{Fact, Manager, Socket};
use crate::broker::peercred::{read_peer_credentials, PeerCredentials};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::Path;

const PROBE_TIMEOUT_MS: libc::c_int = 500;

/// One connect attempt.
#[derive(Debug)]
pub(crate) enum Answer {
    Absent,
    Refused,
    Peer(PeerCredentials),
    /// Saturated, slow, or failed. Nothing can be proved either way.
    Unclear,
}

pub(crate) fn connect(path: &Path) -> Answer {
    // SAFETY: socket takes plain integer arguments.
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if raw < 0 {
        return Answer::Unclear;
    }
    // SAFETY: socket returned a new descriptor that nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: fcntl reads and sets flags on an open descriptor.
    let nonblocking = unsafe {
        let flags = libc::fcntl(fd.as_raw_fd(), libc::F_GETFL);
        flags >= 0 && libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) == 0
    };
    if !nonblocking {
        return Answer::Unclear;
    }
    // SAFETY: zeroed sockaddr_un is a valid address to fill in.
    let mut addr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() >= addr.sun_path.len() {
        return Answer::Unclear;
    }
    for (slot, byte) in addr.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    // SAFETY: addr is a filled sockaddr_un and the length matches it.
    let rc = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&addr as *const libc::sockaddr_un).cast(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::ENOENT) => return Answer::Absent,
            Some(libc::ECONNREFUSED) => return Answer::Refused,
            Some(libc::EINPROGRESS) if connected_in_time(&fd) => {}
            _ => return Answer::Unclear,
        }
    }
    match read_peer_credentials(&UnixStream::from(fd)) {
        Ok(peer) => Answer::Peer(peer),
        Err(_) => Answer::Unclear,
    }
}

fn connected_in_time(fd: &OwnedFd) -> bool {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLOUT,
        revents: 0,
    };
    // SAFETY: poll reads one pollfd for an open descriptor.
    if unsafe { libc::poll(&mut poll, 1, PROBE_TIMEOUT_MS) } != 1 {
        return false;
    }
    let mut error: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: SO_ERROR writes one int through valid pointers.
    let rc = unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            (&mut error as *mut libc::c_int).cast(),
            &mut len,
        )
    };
    rc == 0 && error == 0
}

/// Judge one answer against the manager. Linux refuses only when nothing
/// listens. A saturated macOS listener refuses too, so refusal is unclear there.
pub(crate) fn judge(answer: Answer, owner: u32, manager: &Fact<Manager>) -> Fact<Socket> {
    match answer {
        Answer::Absent => Fact::Known(Socket::Absent),
        Answer::Refused if cfg!(target_os = "macos") => {
            Fact::Unknown("the socket refused a connection, which macOS also does when saturated")
        }
        Answer::Refused => Fact::Known(Socket::Stale),
        Answer::Unclear => Fact::Unknown("the socket did not accept a connection in time"),
        Answer::Peer(peer) => match manager {
            Fact::Known(manager) => {
                let ours = peer.uid == owner
                    && manager.pid.is_some_and(|pid| i64::from(pid) == i64::from(peer.pid));
                Fact::Known(if ours { Socket::Managed } else { Socket::Foreign })
            }
            _ => Fact::Unknown("the socket answered, but the service pid is unknown"),
        },
    }
}
