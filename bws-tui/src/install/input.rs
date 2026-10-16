//! Token input for install: bounded private stdin or one hidden terminal line.
//! The token never comes from argv or the environment, and every buffer is
//! zeroed on drop.

use super::acl;
use super::native_io::{stopping, Owned, TICK};
use super::sys::fstat;
use crate::broker::token::{read_bounded, TOKEN_LIMIT};
use anyhow::{anyhow, bail, Context, Result};
use secrecy::SecretString;
use std::ffi::{CStr, OsStr};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, IsTerminal};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use zeroize::Zeroizing;

pub(crate) trait TokenSource {
    fn read(&mut self) -> Result<SecretString>;
}

/// The operator's stdin: a hidden prompt on a terminal, otherwise a private pipe or file.
pub(crate) struct OperatorInput;

impl TokenSource for OperatorInput {
    fn read(&mut self) -> Result<SecretString> {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            eprint!("Bitwarden machine account token (input hidden): ");
            let token = hidden_line(stdin.as_fd());
            eprintln!();
            return token;
        }
        check_private(stdin.as_fd())?;
        read_bounded(&mut stdin.lock())
    }
}

/// Stdin must be a descriptor no other user can read from. A socket or an
/// anonymous pipe reaches only the processes holding it. A named FIFO or a
/// regular file must be this user's, with no group or other bits and no
/// extended ACL grant. Privacy that cannot be proven is refused.
pub(crate) fn check_private(fd: BorrowedFd<'_>) -> Result<()> {
    let stat = fstat(fd).context("cannot inspect stdin")?;
    let kind = stat.st_mode & libc::S_IFMT;
    if kind == libc::S_IFSOCK || (kind == libc::S_IFIFO && anonymous_pipe(fd)?) {
        return Ok(());
    }
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    let owner_only = stat.st_uid == euid && stat.st_mode as u32 & 0o077 == 0;
    if !matches!(kind, libc::S_IFIFO | libc::S_IFREG) || !owner_only {
        bail!("the token must come from a pipe, an owner-only file or the terminal");
    }
    match acl::grants(fd) {
        Ok(false) => Ok(()),
        Ok(true) => bail!("the token input has an extended ACL grant"),
        Err(_) => bail!("cannot prove that the token input is private"),
    }
}

/// A pipe(2) pipe lives on no filesystem, so no other process can open it by
/// name. Linux reports pipefs for it, and macOS refuses fstatfs with EINVAL.
fn anonymous_pipe(fd: BorrowedFd<'_>) -> Result<bool> {
    // SAFETY: zeroed statfs is a valid out buffer for fstatfs on an open descriptor.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    let found = unsafe { libc::fstatfs(fd.as_raw_fd(), &mut stat) } == 0;
    #[cfg(target_os = "linux")]
    if found {
        const PIPEFS_MAGIC: libc::c_long = 0x5049_5045;
        return Ok(stat.f_type == PIPEFS_MAGIC);
    }
    #[cfg(target_os = "macos")]
    if found || std::io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL) {
        return Ok(!found);
    }
    bail!("cannot tell whether stdin is a private pipe")
}

/// Read one line from a terminal with echo off, restoring the mode afterwards.
/// Cleanup ownership comes before echo goes off, so a stop signal is held.
/// The wait sees it, the mode is restored, and the owner's drop delivers it.
pub(crate) fn hidden_line(terminal: BorrowedFd<'_>) -> Result<SecretString> {
    let owned = Owned::take()?;
    let private = private_terminal(terminal)?;
    let fd = terminal.as_raw_fd();
    // SAFETY: zeroed termios is a valid out buffer for tcgetattr.
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut saved) } != 0 {
        bail!("cannot read the terminal mode");
    }
    let mut quiet = saved;
    quiet.c_lflag &= !(libc::ECHO | libc::ECHONL);
    // SAFETY: tcsetattr with a mode derived from the terminal's own.
    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &quiet) } != 0 {
        bail!("cannot turn terminal echo off");
    }
    let line = read_line(private.as_raw_fd());
    // SAFETY: restores the mode saved above on every path.
    let restored = unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &saved) } == 0;
    let token = match restored {
        true => line.and_then(|line| read_bounded(&mut line.as_slice())),
        false => Err(anyhow!("cannot restore the terminal mode")),
    };
    if stopping() {
        // The held signal ends the process before any error could return.
        if !restored {
            eprintln!("cannot restore the terminal mode");
        }
        drop(token);
        drop(owned);
        bail!("a stop signal arrived");
    }
    token
}

/// A nonblocking read handle on the same terminal. Select readiness does not
/// reserve input, so a blocking read could outwait a held stop signal. The
/// flag lives on this handle's own open file description. A dup would share,
/// and change, the caller's flags. The reopened node must be the same device.
pub(crate) fn private_terminal(terminal: BorrowedFd<'_>) -> Result<File> {
    let mut name = [0 as libc::c_char; 1024];
    // SAFETY: ttyname_r writes a NUL-terminated name into the live buffer.
    if unsafe { libc::ttyname_r(terminal.as_raw_fd(), name.as_mut_ptr(), name.len()) } != 0 {
        bail!("cannot name the terminal");
    }
    // SAFETY: on success the buffer holds a NUL-terminated name.
    let path = OsStr::from_bytes(unsafe { CStr::from_ptr(name.as_ptr()) }.to_bytes());
    let private = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(path)
        .context("cannot open a private handle on the terminal")?;
    let (want, got) = (fstat(terminal)?, fstat(private.as_fd())?);
    let device = |stat: &libc::stat| stat.st_mode & libc::S_IFMT == libc::S_IFCHR;
    let node = |stat: &libc::stat| (stat.st_dev, stat.st_ino, stat.st_rdev);
    if !device(&want) || !device(&got) || node(&want) != node(&got) {
        bail!("the reopened terminal is not the input terminal");
    }
    Ok(private)
}

/// Bytes up to the first newline. A line longer than the token limit is refused.
/// `fd` is nonblocking, so input taken by another reader after the wait
/// returns to the wait instead of blocking.
fn read_line(fd: libc::c_int) -> Result<Zeroizing<Vec<u8>>> {
    let mut line = Zeroizing::new(vec![0u8; TOKEN_LIMIT + 1]);
    let mut filled = 0;
    while filled < line.len() {
        if !input_ready(fd)? {
            continue;
        }
        // SAFETY: reads into the unfilled tail of a buffer this function owns.
        let read =
            unsafe { libc::read(fd, line[filled..].as_mut_ptr().cast(), line.len() - filled) };
        match read {
            0 => break,
            read if read < 0
                && matches!(
                    std::io::Error::last_os_error().kind(),
                    ErrorKind::Interrupted | ErrorKind::WouldBlock
                ) =>
            {
                continue
            }
            read if read < 0 => bail!("cannot read the terminal"),
            read => filled += read as usize,
        }
        if let Some(end) = line[..filled].iter().position(|byte| *byte == b'\n') {
            line.truncate(end);
            return Ok(line);
        }
    }
    if filled > TOKEN_LIMIT {
        bail!("token is larger than {TOKEN_LIMIT} bytes");
    }
    line.truncate(filled);
    Ok(line)
}

/// Wait up to one tick for terminal input. A held stop signal cancels the
/// read. macOS poll(2) does not support devices, so this uses select(2).
fn input_ready(fd: libc::c_int) -> Result<bool> {
    if stopping() {
        bail!("a stop signal arrived");
    }
    if fd >= libc::FD_SETSIZE as libc::c_int {
        bail!("the terminal descriptor is too large to wait on");
    }
    // SAFETY: a zeroed fd_set is a valid set, and fd is below FD_SETSIZE.
    let mut ready: libc::fd_set = unsafe { std::mem::zeroed() };
    unsafe { libc::FD_SET(fd, &mut ready) };
    let mut wait = libc::timeval {
        tv_sec: 0,
        tv_usec: TICK.as_micros() as libc::suseconds_t,
    };
    let null = std::ptr::null_mut();
    // SAFETY: every set and the timeout are live for the call.
    match unsafe { libc::select(fd + 1, &mut ready, null, null, &mut wait) } {
        found if found > 0 => Ok(true),
        0 => Ok(false),
        _ if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted => Ok(false),
        _ => bail!("cannot wait for the terminal"),
    }
}
