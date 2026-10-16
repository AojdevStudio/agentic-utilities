//! Checks that a passed descriptor is the requesting process's own
//! controlling terminal and that the process is in the foreground. All
//! evidence comes from the kernel: `fstat`, `fcntl`, and the peer's process
//! record. A non-controlling process cannot ask the terminal itself for its
//! session or foreground group, so the process record is the source.

use super::operation::{DescriptorProblem, Refusal};
use super::peercred::PeerCredentials;
use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

/// The peer's controlling terminal as the kernel records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PeerTerminal {
    device: u64,
    foreground: i64,
    group: i64,
}

/// Require a read-write terminal owned by the peer that is the peer's
/// controlling terminal, with the peer in its foreground process group.
pub(crate) fn check_terminal(
    fd: BorrowedFd<'_>,
    peer: PeerCredentials,
) -> Result<(), DescriptorProblem> {
    let stat = fstat(fd).ok_or(DescriptorProblem::NotATerminal)?;
    // SAFETY: isatty only inspects the descriptor.
    let tty = unsafe { libc::isatty(fd.as_raw_fd()) } == 1;
    if stat.st_mode & libc::S_IFMT != libc::S_IFCHR || !tty {
        return Err(DescriptorProblem::NotATerminal);
    }
    if access_mode(fd) != Some(libc::O_RDWR) {
        return Err(DescriptorProblem::NotReadWrite);
    }
    if stat.st_uid != peer.uid {
        return Err(DescriptorProblem::WrongOwner);
    }
    let terminal = peer_terminal(peer.pid).ok_or(DescriptorProblem::NotControlling)?;
    if terminal.device != device_number(&stat) {
        return Err(DescriptorProblem::NotControlling);
    }
    if terminal.foreground <= 0 || terminal.foreground != terminal.group {
        return Err(DescriptorProblem::NotForeground);
    }
    Ok(())
}

/// The daemon's own open of a checked terminal. A passed descriptor shares
/// its open file description, and so its O_NONBLOCK flag, with the client.
/// A fresh open does not. The received descriptor stays open until the new
/// one proves to be the same device and passes the same kernel checks, so
/// the device cannot be swapped underneath. There is no fallback to the
/// received descriptor.
pub(crate) fn reopen_terminal(
    received: OwnedFd,
    peer: PeerCredentials,
) -> Result<OwnedFd, Refusal> {
    let problem = |problem| Refusal::Descriptors { problem };
    check_terminal(received.as_fd(), peer).map_err(problem)?;
    let path = terminal_path(received.as_fd()).ok_or(Refusal::TerminalFailed)?;
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC | NOFOLLOW;
    // SAFETY: open of a NUL-terminated path; a new descriptor is owned below.
    let fd = unsafe { libc::open(path.as_ptr(), flags) };
    if fd < 0 {
        return Err(Refusal::TerminalFailed);
    }
    // SAFETY: fd is new and owned only here.
    let own = unsafe { OwnedFd::from_raw_fd(fd) };
    let same = match (fstat(received.as_fd()), fstat(own.as_fd())) {
        (Some(old), Some(new)) => same_device(&old, &new),
        _ => false,
    };
    if !same {
        return Err(Refusal::TerminalFailed);
    }
    check_terminal(own.as_fd(), peer).map_err(problem)?;
    drop(received);
    Ok(own)
}

fn same_device(old: &libc::stat, new: &libc::stat) -> bool {
    old.st_mode & libc::S_IFMT == new.st_mode & libc::S_IFMT
        && old.st_dev == new.st_dev
        && old.st_ino == new.st_ino
        && old.st_rdev == new.st_rdev
        && old.st_uid == new.st_uid
}

/// Linux opens the exact device through the descriptor's magic link, which
/// follows a symlink by design.
#[cfg(target_os = "linux")]
const NOFOLLOW: libc::c_int = 0;
#[cfg(not(target_os = "linux"))]
const NOFOLLOW: libc::c_int = libc::O_NOFOLLOW;

#[cfg(target_os = "linux")]
fn terminal_path(fd: BorrowedFd<'_>) -> Option<CString> {
    CString::new(format!("/proc/self/fd/{}", fd.as_raw_fd())).ok()
}

/// macOS `/dev/fd/N` is a duplicate that shares the description, so the
/// device's own name is opened instead.
#[cfg(not(target_os = "linux"))]
fn terminal_path(fd: BorrowedFd<'_>) -> Option<CString> {
    let mut name = [0u8; 256];
    // SAFETY: ttyname_r writes at most `name.len()` bytes into `name`.
    let failed = unsafe { libc::ttyname_r(fd.as_raw_fd(), name.as_mut_ptr().cast(), name.len()) };
    if failed != 0 {
        return None;
    }
    std::ffi::CStr::from_bytes_until_nul(&name)
        .ok()
        .map(ToOwned::to_owned)
}

/// A value input is a pipe or file the daemon reads. Never a terminal.
pub(crate) fn check_value_input(fd: BorrowedFd<'_>) -> Result<(), DescriptorProblem> {
    // SAFETY: isatty only inspects the descriptor.
    if unsafe { libc::isatty(fd.as_raw_fd()) } == 1 {
        return Err(DescriptorProblem::ValueInputIsTerminal);
    }
    match access_mode(fd) {
        Some(libc::O_RDONLY | libc::O_RDWR) => Ok(()),
        _ => Err(DescriptorProblem::NotReadWrite),
    }
}

fn fstat(fd: BorrowedFd<'_>) -> Option<libc::stat> {
    // SAFETY: zeroed stat is a valid out buffer for fstat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat writes into a valid buffer.
    (unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } == 0).then_some(stat)
}

/// `st_rdev` is unsigned on Linux and a signed 32-bit `dev_t` on macOS.
#[cfg(target_os = "macos")]
fn device_number(stat: &libc::stat) -> u64 {
    u64::from(stat.st_rdev as u32)
}

#[cfg(not(target_os = "macos"))]
fn device_number(stat: &libc::stat) -> u64 {
    stat.st_rdev
}

fn access_mode(fd: BorrowedFd<'_>) -> Option<i32> {
    // SAFETY: F_GETFL only reads descriptor flags.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    (flags >= 0).then_some(flags & libc::O_ACCMODE)
}

/// Linux `/proc/<pid>/stat`: fields after the command name are state, ppid,
/// pgrp, session, tty_nr, and tpgid. `tty_nr` uses the kernel's device encoding.
#[cfg(target_os = "linux")]
fn peer_terminal(pid: i32) -> Option<PeerTerminal> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_proc_stat(&stat)
}

#[cfg(target_os = "linux")]
fn parse_proc_stat(stat: &str) -> Option<PeerTerminal> {
    let fields: Vec<&str> = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    let group: i64 = fields.get(2)?.parse().ok()?;
    let tty_nr: u64 = fields.get(4)?.parse().ok()?;
    let foreground: i64 = fields.get(5)?.parse().ok()?;
    if tty_nr == 0 {
        return None;
    }
    let major = ((tty_nr >> 8) & 0xfff) as u32;
    let minor = ((tty_nr & 0xff) | ((tty_nr >> 12) & 0xfff00)) as u32;
    Some(PeerTerminal {
        device: libc::makedev(major, minor),
        foreground,
        group,
    })
}

/// macOS `proc_pidinfo(PROC_PIDTBSDINFO)`: the controlling terminal device,
/// its foreground process group, and the process group. No terminal is `NODEV`.
#[cfg(target_os = "macos")]
fn peer_terminal(pid: i32) -> Option<PeerTerminal> {
    // SAFETY: zeroed proc_bsdinfo is a valid out buffer.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if written != size || info.e_tdev == u32::MAX {
        return None;
    }
    Some(PeerTerminal {
        device: u64::from(info.e_tdev),
        foreground: i64::from(info.e_tpgid),
        group: i64::from(info.pbi_pgid),
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn peer_terminal(_pid: i32) -> Option<PeerTerminal> {
    None
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn proc_stat_parsing_survives_hostile_command_names() {
        // pts/3 is major 136, minor 3, encoded as 136 << 8 | 3.
        let stat = "4242 (evil) 1 2 (x) S 1 4242 4242 34819 4242 4194560 0 0";
        assert_eq!(
            parse_proc_stat(stat),
            Some(PeerTerminal {
                device: libc::makedev(136, 3),
                foreground: 4242,
                group: 4242,
            })
        );
        let no_tty = "7 (daemon) S 1 7 7 0 -1 4194560";
        assert_eq!(parse_proc_stat(no_tty), None);
    }
}
