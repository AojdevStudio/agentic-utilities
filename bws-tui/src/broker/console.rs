//! The daemon side of a passed terminal: hidden line input that the human
//! or a client disconnect can cancel, value display escaped against terminal
//! control injection, and OSC 52. Only a checked terminal becomes a `Console`.

use super::approve::APPROVAL_TIMEOUT;
pub(crate) use super::fdio::Interrupt;
use super::fdio::Passed;
use base64::Engine;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// Largest value written to the terminal clipboard.
pub(crate) const MAX_OSC52_VALUE: usize = 128 * 1024;

pub(crate) struct Console<'a> {
    tty: Passed,
    socket: &'a UnixStream,
    timeout: Duration,
}

/// Restores the saved terminal mode on every exit path.
/// It holds the raw descriptor because the console stays open for its whole life.
struct ModeGuard {
    fd: RawFd,
    saved: libc::termios,
}

impl Drop for ModeGuard {
    fn drop(&mut self) {
        // SAFETY: restores a termios read from this same, still open descriptor.
        unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved) };
    }
}

impl<'a> Console<'a> {
    /// Wrap a descriptor that already passed `check_terminal`. Activity on
    /// `socket` cancels any wait.
    pub(crate) fn new(fd: OwnedFd, socket: &'a UnixStream) -> io::Result<Self> {
        Ok(Self {
            tty: Passed::new(fd)?,
            socket,
            timeout: APPROVAL_TIMEOUT,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub(crate) fn fd(&self) -> BorrowedFd<'_> {
        self.tty.as_fd()
    }

    /// Write daemon text. Names from the vault are escaped first.
    pub(crate) fn say(&mut self, text: &str) -> Result<(), Interrupt> {
        let deadline = Instant::now() + self.timeout;
        self.tty.write_all(text.as_bytes(), self.socket, deadline)
    }

    /// Read one line with echo, line editing, and signals off. Bytes are kept
    /// in a zeroizing buffer that never grows past `limit`, so it never copies.
    pub(crate) fn read_hidden(
        &mut self,
        prompt: &str,
        limit: usize,
    ) -> Result<Zeroizing<Vec<u8>>, Interrupt> {
        let _mode = self.hidden_mode().map_err(|_| Interrupt::Io)?;
        // SAFETY: discards typeahead queued before the prompt appeared.
        if unsafe { libc::tcflush(self.tty.as_fd().as_raw_fd(), libc::TCIFLUSH) } != 0 {
            return Err(Interrupt::Io);
        }
        self.say(prompt)?;
        let mut line = Zeroizing::new(Vec::with_capacity(limit));
        let result = self.collect(&mut line, limit);
        let ended = self.say("\r\n");
        result?;
        ended?;
        Ok(line)
    }

    fn collect(&mut self, line: &mut Vec<u8>, limit: usize) -> Result<(), Interrupt> {
        let deadline = Instant::now() + self.timeout;
        let mut chunk = Zeroizing::new([0u8; 256]);
        loop {
            let read = self.tty.read(&mut chunk[..], self.socket, deadline)?;
            if read == 0 {
                return Err(Interrupt::Cancelled);
            }
            for &byte in &chunk[..read] {
                match byte {
                    b'\r' | b'\n' => return Ok(()),
                    0x03 => return Err(Interrupt::Cancelled),
                    0x04 if line.is_empty() => return Err(Interrupt::Cancelled),
                    0x7f | 0x08 => {
                        if let Some(last) = line.last_mut() {
                            last.zeroize();
                            line.pop();
                        }
                    }
                    0x15 => line.zeroize(),
                    _ if line.len() == limit => return Err(Interrupt::TooLong),
                    _ => line.push(byte),
                }
            }
        }
    }

    fn hidden_mode(&self) -> io::Result<ModeGuard> {
        let fd = self.tty.as_fd().as_raw_fd();
        // SAFETY: zeroed termios is a valid out buffer for tcgetattr.
        let mut saved: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: tcgetattr writes into a valid buffer.
        if unsafe { libc::tcgetattr(fd, &mut saved) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut hidden = saved;
        hidden.c_lflag &=
            !(libc::ECHO | libc::ECHOE | libc::ECHOK | libc::ECHONL | libc::ICANON | libc::ISIG);
        hidden.c_lflag &= !libc::IEXTEN;
        hidden.c_cc[libc::VMIN] = 1;
        hidden.c_cc[libc::VTIME] = 0;
        // SAFETY: sets a termios derived from this descriptor's own settings.
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ModeGuard { fd, saved })
    }

    /// Show a value on its own screen, wait for Enter, then clear it, so the
    /// value never stays in scrollback. The clear runs on every path.
    pub(crate) fn show_value(&mut self, title: &str, value: &[u8]) -> Result<(), Interrupt> {
        // Reserved up front so the buffer never reallocates and leaves a copy behind.
        let room = 64 + MAX_ESCAPE_GROWTH * (title.len() + value.len());
        let mut screen = Zeroizing::new(String::with_capacity(room));
        screen.push_str("\x1b[?1049h\x1b[H\x1b[2J");
        escape_into(&mut screen, title.as_bytes());
        screen.push_str("\r\n\r\n");
        escape_into(&mut screen, value);
        screen.push_str("\r\n\r\n");
        let shown =
            self.say(&screen)
                .and_then(|()| match self.read_hidden("(press Enter to hide)", 0) {
                    Ok(_) | Err(Interrupt::TooLong) | Err(Interrupt::Cancelled) => Ok(()),
                    Err(interrupt) => Err(interrupt),
                });
        let cleared = self.say("\x1b[2J\x1b[?1049l");
        shown.and(cleared)
    }

    /// Send a bounded OSC 52 clipboard write. The terminal cannot confirm it.
    pub(crate) fn osc52(&mut self, value: &[u8]) -> Result<(), Interrupt> {
        if value.len() > MAX_OSC52_VALUE {
            return Err(Interrupt::TooLong);
        }
        let mut sequence = Zeroizing::new(String::with_capacity(16 + value.len().div_ceil(3) * 4));
        sequence.push_str("\x1b]52;c;");
        base64::engine::general_purpose::STANDARD.encode_string(value, &mut sequence);
        sequence.push('\x07');
        self.say(&sequence)
    }
}

/// One input byte becomes at most this many output bytes in [`escape_into`].
const MAX_ESCAPE_GROWTH: usize = 6;

/// Make bytes safe to print. Tabs and newlines stay. Every other control
/// character, invalid UTF-8 byte, and backslash becomes a visible escape, so
/// a stored value cannot move the cursor, retitle the window, or set the clipboard.
pub(crate) fn escape_into(out: &mut String, bytes: &[u8]) {
    use std::fmt::Write as _;
    for chunk in bytes.utf8_chunks() {
        for ch in chunk.valid().chars() {
            let _ = match ch {
                '\n' => out.write_str("\r\n"),
                '\t' => out.write_char('\t'),
                '\\' => out.write_str("\\\\"),
                c if c.is_control() => write!(out, "\\u{{{:x}}}", c as u32),
                c => out.write_char(c),
            };
        }
        for byte in chunk.invalid() {
            let _ = write!(out, "\\x{byte:02x}");
        }
    }
}

/// [`escape_into`] for vault metadata such as key and project names.
pub(crate) fn escape(bytes: &[u8]) -> String {
    let mut out = String::new();
    escape_into(&mut out, bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escape_neutralizes_terminal_control() {
        let hostile = b"a\x1b]52;c;ZXZpbA==\x07b\\c\td\ne\xffend\xc2\x9b";
        assert_eq!(
            escape(hostile),
            "a\\u{1b}]52;c;ZXZpbA==\\u{7}b\\\\c\td\r\ne\\xffend\\u{9b}"
        );
    }
}
