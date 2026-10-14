//! Descriptor passing over the broker socket with `SCM_RIGHTS`.
//!
//! Rights travel only with the first byte of a frame. Rights on any later
//! byte, truncated rights, and foreign control messages are violations. A
//! violation closes the descriptors involved and the read still consumes the
//! whole frame, so the connection stays in sync.

use anyhow::{bail, Result};
use std::io::{Error, ErrorKind};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// Room for more rights than any operation takes, so extra rights arrive and
/// get closed here instead of being cut by the kernel.
const RIGHTS_ROOM: usize = 8;

/// Why the rights on one frame were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightsViolation {
    Truncated,
    LateRights,
    ForeignControl,
}

/// A control buffer with the alignment `cmsghdr` needs.
#[repr(C, align(8))]
struct ControlBuffer([u8; 256]);

fn space(fds: usize) -> usize {
    // SAFETY: CMSG_SPACE only computes a size.
    unsafe { libc::CMSG_SPACE((fds * std::mem::size_of::<RawFd>()) as libc::c_uint) as usize }
}

/// Send `bytes` with `fds` attached to the first byte, then the rest.
pub(crate) fn send_with_rights(
    socket: BorrowedFd<'_>,
    bytes: &[u8],
    fds: &[BorrowedFd<'_>],
) -> Result<()> {
    let Some((first, rest)) = bytes.split_first() else {
        bail!("cannot send an empty frame");
    };
    if space(fds.len()) > std::mem::size_of::<ControlBuffer>() {
        bail!("too many descriptors for one frame");
    }
    let mut control = ControlBuffer([0; 256]);
    let mut iov = libc::iovec {
        iov_base: (first as *const u8).cast_mut().cast(),
        iov_len: 1,
    };
    // SAFETY: zeroed msghdr is valid; every pointer set below outlives sendmsg.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    if !fds.is_empty() {
        msg.msg_control = control.0.as_mut_ptr().cast();
        msg.msg_controllen = space(fds.len()) as _;
        // SAFETY: the buffer holds one header plus `fds.len()` descriptors.
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&msg);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len =
                libc::CMSG_LEN((fds.len() * std::mem::size_of::<RawFd>()) as libc::c_uint) as _;
            let data = libc::CMSG_DATA(header).cast::<RawFd>();
            for (index, fd) in fds.iter().enumerate() {
                data.add(index).write_unaligned(fd.as_raw_fd());
            }
        }
    }
    loop {
        // SAFETY: msg points at live buffers for the duration of the call.
        let sent = unsafe { libc::sendmsg(socket.as_raw_fd(), &msg, 0) };
        if sent == 1 {
            break;
        }
        let error = Error::last_os_error();
        if sent < 0 && error.kind() == ErrorKind::Interrupted {
            continue;
        }
        return Err(error.into());
    }
    write_all(socket, rest)
}

fn write_all(socket: BorrowedFd<'_>, mut bytes: &[u8]) -> Result<()> {
    while !bytes.is_empty() {
        // SAFETY: writes from a live slice.
        let written =
            unsafe { libc::write(socket.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        if written < 0 {
            let error = Error::last_os_error();
            if error.kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

/// Read exactly `buf.len()` bytes. Rights are kept only on the first byte when
/// `first` is true. Every other right is closed and reported as a violation.
pub(crate) fn recv_exact(
    socket: BorrowedFd<'_>,
    buf: &mut [u8],
    first: bool,
) -> Result<(Vec<OwnedFd>, Option<RightsViolation>)> {
    let mut kept = Vec::new();
    let mut violation = None;
    let mut filled = 0;
    while filled < buf.len() {
        let (read, fds, problem) = recv_some(socket, &mut buf[filled..])?;
        let late = !fds.is_empty() && !(first && filled == 0);
        match (problem, late) {
            (Some(problem), _) => violation = violation.or(Some(problem)),
            (None, true) => violation = violation.or(Some(RightsViolation::LateRights)),
            (None, false) => kept.extend(fds),
        }
        if read == 0 {
            return Err(Error::from(ErrorKind::UnexpectedEof).into());
        }
        filled += read;
    }
    if violation.is_some() {
        kept.clear();
    }
    Ok((kept, violation))
}

/// One `recvmsg`. Received descriptors become owned and close-on-exec at once.
fn recv_some(
    socket: BorrowedFd<'_>,
    buf: &mut [u8],
) -> Result<(usize, Vec<OwnedFd>, Option<RightsViolation>)> {
    let mut control = ControlBuffer([0; 256]);
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    // SAFETY: zeroed msghdr is valid; every pointer set below outlives recvmsg.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = space(RIGHTS_ROOM) as _;
    #[cfg(target_os = "linux")]
    let flags = libc::MSG_CMSG_CLOEXEC;
    #[cfg(not(target_os = "linux"))]
    let flags = 0;
    let read = loop {
        // SAFETY: msg points at live buffers for the duration of the call.
        let read = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, flags) };
        if read >= 0 {
            break read as usize;
        }
        let error = Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error.into());
        }
    };
    let mut fds = Vec::new();
    let mut problem = (msg.msg_flags & libc::MSG_CTRUNC != 0).then_some(RightsViolation::Truncated);
    // SAFETY: the kernel filled `control` and set msg_controllen; the cmsg
    // macros walk only within that length.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&msg);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let bytes = (*header).cmsg_len as usize - libc::CMSG_LEN(0) as usize;
                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                for index in 0..bytes / std::mem::size_of::<RawFd>() {
                    let fd = OwnedFd::from_raw_fd(data.add(index).read_unaligned());
                    #[cfg(not(target_os = "linux"))]
                    libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
                    fds.push(fd);
                }
            } else {
                problem = problem.or(Some(RightsViolation::ForeignControl));
            }
            header = libc::CMSG_NXTHDR(&msg, header);
        }
    }
    if problem.is_some() {
        fds.clear();
    }
    Ok((read, fds, problem))
}
