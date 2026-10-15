//! Token input for a new install: a private pipe or file on stdin, or a
//! hidden terminal prompt. Never argv, the environment, or a log.

use super::apply::TokenSource;
use crate::broker::console::Console;
use crate::broker::request::RequestControl;
use crate::broker::token::{read_secret, secret_from_bytes, TOKEN_MAX_BYTES};
use anyhow::{bail, Result};
use secrecy::SecretString;
use std::io::IsTerminal;
use std::os::fd::{FromRawFd, OwnedFd};

pub(crate) struct ProcessInput;

impl TokenSource for ProcessInput {
    fn read_token(&mut self) -> Result<SecretString> {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            return read_terminal();
        }
        // SAFETY: zeroed stat is a valid out buffer for fstat on stdin.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fstat writes one stat for descriptor 0.
        if unsafe { libc::fstat(0, &mut stat) } != 0 {
            bail!("could not inspect the token input");
        }
        let kind = stat.st_mode & libc::S_IFMT;
        let private_file = kind == libc::S_IFREG && stat.st_mode & 0o077 == 0;
        if kind != libc::S_IFIFO && kind != libc::S_IFSOCK && !private_file {
            bail!("pipe the token, redirect it from a private file, or run install on a terminal");
        }
        read_secret(&mut stdin.lock())
    }
}

/// Prompt on the controlling terminal with echo, line editing, and signals off.
fn read_terminal() -> Result<SecretString> {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC;
    // SAFETY: open reads one valid C string.
    let raw = unsafe { libc::open(c"/dev/tty".as_ptr(), flags) };
    if raw < 0 {
        bail!("could not open the terminal for the token prompt");
    }
    // SAFETY: open returned a new descriptor that nothing else owns.
    let tty = unsafe { OwnedFd::from_raw_fd(raw) };
    let control = RequestControl::detached();
    let mut console = Console::new(tty, &control);
    let Ok(line) = console.read_hidden("BWS access token: ", TOKEN_MAX_BYTES) else {
        bail!("the token prompt was cancelled or failed");
    };
    secret_from_bytes(&line)
}
