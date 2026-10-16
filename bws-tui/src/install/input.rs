//! Token input for install: bounded private stdin or one hidden terminal line.
//! The token never comes from argv or the environment, and every buffer is
//! zeroed on drop.

use super::sys::fstat;
use crate::broker::token::{read_bounded, TOKEN_LIMIT};
use anyhow::{bail, Context, Result};
use secrecy::SecretString;
use std::io::{ErrorKind, IsTerminal};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
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

/// Stdin may be a pipe, a socket or a file only its owner can read.
pub(crate) fn check_private(fd: BorrowedFd<'_>) -> Result<()> {
    let stat = fstat(fd).context("cannot inspect stdin")?;
    let kind = stat.st_mode & libc::S_IFMT;
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    let private_file =
        kind == libc::S_IFREG && stat.st_uid == euid && stat.st_mode as u32 & 0o077 == 0;
    if kind == libc::S_IFIFO || kind == libc::S_IFSOCK || private_file {
        return Ok(());
    }
    bail!("the token must come from a pipe, an owner-only file or the terminal")
}

/// Read one line from a terminal with echo off, restoring the mode afterwards.
pub(crate) fn hidden_line(terminal: BorrowedFd<'_>) -> Result<SecretString> {
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
    let line = read_line(fd);
    // SAFETY: restores the mode saved above on every path.
    let restored = unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &saved) } == 0;
    let line = line?;
    if !restored {
        bail!("cannot restore the terminal mode");
    }
    read_bounded(&mut line.as_slice())
}

/// Bytes up to the first newline. A line longer than the token limit is refused.
fn read_line(fd: libc::c_int) -> Result<Zeroizing<Vec<u8>>> {
    let mut line = Zeroizing::new(vec![0u8; TOKEN_LIMIT + 1]);
    let mut filled = 0;
    while filled < line.len() {
        // SAFETY: reads into the unfilled tail of a buffer this function owns.
        let read =
            unsafe { libc::read(fd, line[filled..].as_mut_ptr().cast(), line.len() - filled) };
        match read {
            0 => break,
            read if read < 0
                && std::io::Error::last_os_error().kind() == ErrorKind::Interrupted =>
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
