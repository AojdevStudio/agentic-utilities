//! Peer identity read from the kernel. The client cannot assert these values.

use anyhow::{Context, Result};
use std::os::unix::net::UnixStream;

/// Caller identity for one connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
    pub pid: i32,
}

pub(crate) fn read_peer_credentials(stream: &UnixStream) -> Result<PeerCredentials> {
    #[cfg(target_os = "macos")]
    {
        read_peer_credentials_macos(stream)
    }
    #[cfg(target_os = "linux")]
    {
        read_peer_credentials_linux(stream)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = stream;
        anyhow::bail!("peer credentials unsupported on this platform")
    }
}

#[cfg(target_os = "macos")]
fn read_peer_credentials_macos(stream: &UnixStream) -> Result<PeerCredentials> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: getpeereid writes the peer uid and gid through valid pointers.
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("getpeereid failed");
    }
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: LOCAL_PEERPID writes one pid_t and updates the length through valid pointers.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("LOCAL_PEERPID failed");
    }
    Ok(PeerCredentials { uid, gid, pid })
}

#[cfg(target_os = "linux")]
fn read_peer_credentials_linux(stream: &UnixStream) -> Result<PeerCredentials> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    // SAFETY: zeroed ucred is a valid buffer for SO_PEERCRED.
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: SO_PEERCRED writes cred and updates the length through valid pointers.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("SO_PEERCRED failed");
    }
    Ok(PeerCredentials {
        uid: cred.uid,
        gid: cred.gid,
        pid: cred.pid,
    })
}
