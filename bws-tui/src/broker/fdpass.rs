//! Descriptor passing over the broker socket with `SCM_RIGHTS`.
//!
//! Rights travel only with the first byte of a frame. More rights than any
//! operation takes, rights on any later byte, and foreign control messages are
//! violations. A violation closes the descriptors involved and the read still
//! consumes the whole frame, so the connection stays in sync.
//!
//! macOS installs every passed descriptor before it copies the control data,
//! then cuts that data without lowering `cmsg_len`. So the buffer holds more
//! rights than either kernel sends, parsing trusts only the bytes the kernel
//! returned, and truncated control data drops the connection.

use anyhow::{bail, Result};
use std::io::{Error, ErrorKind};
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// The most descriptors any operation takes. A frame with more is refused.
pub(crate) const MAX_RIGHTS: usize = 2;

/// Why the rights on one frame were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightsViolation {
    TooMany,
    LateRights,
    ForeignControl,
}

/// Linux sends at most 253 rights per message and XNU at most 512, which
/// need about 1 KiB and 2 KiB of control data.
const CONTROL_ROOM: usize = 4096;

/// A control buffer with the alignment `cmsghdr` needs.
#[repr(C, align(8))]
struct ControlBuffer([u8; CONTROL_ROOM]);

fn space(bytes: usize) -> usize {
    // SAFETY: CMSG_SPACE only computes a size.
    unsafe { libc::CMSG_SPACE(bytes as libc::c_uint) as usize }
}

fn header_len() -> usize {
    // SAFETY: CMSG_LEN only computes a size.
    unsafe { libc::CMSG_LEN(0) as usize }
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
    let rights = fds.len() * size_of::<RawFd>();
    if space(rights) > CONTROL_ROOM {
        bail!("too many descriptors for one frame");
    }
    let mut control = ControlBuffer([0; CONTROL_ROOM]);
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
        msg.msg_controllen = space(rights) as _;
        // SAFETY: the buffer holds one header plus `fds.len()` descriptors.
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&msg);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(rights as libc::c_uint) as _;
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

/// One `recvmsg`. Every received descriptor is owned before any refusal.
fn recv_some(
    socket: BorrowedFd<'_>,
    buf: &mut [u8],
) -> Result<(usize, Vec<OwnedFd>, Option<RightsViolation>)> {
    let mut control = ControlBuffer([0; CONTROL_ROOM]);
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    // SAFETY: zeroed msghdr is valid; every pointer set below outlives recvmsg.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = CONTROL_ROOM as _;
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
    let returned = (msg.msg_controllen as usize).min(CONTROL_ROOM);
    let parsed = parse_control(&control.0[..returned]);
    // SAFETY: each number came from control data the kernel wrote in this
    // call, so each is a new descriptor that nothing else owns.
    let fds: Vec<OwnedFd> = parsed
        .fds
        .iter()
        .map(|&fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .collect();
    if parsed.truncated || msg.msg_flags & libc::MSG_CTRUNC != 0 {
        bail!("descriptor control data was truncated");
    }
    close_on_exec(&fds)?;
    let problem = if parsed.foreign {
        Some(RightsViolation::ForeignControl)
    } else if fds.len() > MAX_RIGHTS {
        Some(RightsViolation::TooMany)
    } else {
        None
    };
    match problem {
        Some(problem) => Ok((read, Vec::new(), Some(problem))),
        None => Ok((read, fds, None)),
    }
}

/// Linux already set it with `MSG_CMSG_CLOEXEC`.
#[cfg(target_os = "linux")]
fn close_on_exec(_fds: &[OwnedFd]) -> Result<()> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn close_on_exec(fds: &[OwnedFd]) -> Result<()> {
    for fd in fds {
        // SAFETY: F_SETFD changes only the descriptor flag of an owned descriptor.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(Error::last_os_error().into());
        }
    }
    Ok(())
}

/// The descriptor numbers in received control data.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParsedControl {
    pub fds: Vec<RawFd>,
    /// A header claimed more bytes than the kernel returned, or a length was malformed.
    pub truncated: bool,
    pub foreign: bool,
}

/// Walk the control messages in `control`, which holds exactly the bytes the
/// kernel returned. Only whole descriptor numbers inside those bytes count.
pub(crate) fn parse_control(control: &[u8]) -> ParsedControl {
    let mut parsed = ParsedControl::default();
    let header = header_len();
    let mut offset = 0;
    while offset + size_of::<libc::cmsghdr>() <= control.len() {
        // SAFETY: the header lies inside `control`; read_unaligned needs no alignment.
        let cmsg: libc::cmsghdr =
            unsafe { std::ptr::read_unaligned(control.as_ptr().add(offset).cast()) };
        let claimed = cmsg.cmsg_len as usize;
        if claimed < header {
            parsed.truncated = true;
            break;
        }
        let end = offset.saturating_add(claimed);
        let data = control
            .get(offset + header..end.min(control.len()))
            .unwrap_or(&[]);
        let (numbers, partial) = data.as_chunks::<{ size_of::<RawFd>() }>();
        if end > control.len() || !partial.is_empty() {
            parsed.truncated = true;
        }
        if cmsg.cmsg_level == libc::SOL_SOCKET && cmsg.cmsg_type == libc::SCM_RIGHTS {
            for &bytes in numbers {
                match RawFd::from_ne_bytes(bytes) {
                    fd @ 0.. => parsed.fds.push(fd),
                    _ => parsed.truncated = true,
                }
            }
        } else {
            parsed.foreign = true;
        }
        offset = offset.saturating_add(space(claimed) - space(0));
    }
    parsed
}
