//! The daemon side of a checked terminal: hidden line input that the human
//! or a client disconnect can cancel, value display escaped against terminal
//! control injection, and OSC 52. The console reads and writes the daemon's
//! own open of the terminal, and every wait shares the request's one
//! deadline. Putting the terminal back uses the separate cleanup budget, so
//! it still runs after the client leaves.

use super::fdio::OwnedTty;
pub(crate) use super::request::Interrupt;
use super::request::RequestControl;
use base64::Engine;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use zeroize::{Zeroize, Zeroizing};

/// Largest value written to the terminal clipboard.
pub(crate) const MAX_OSC52_VALUE: usize = 128 * 1024;

pub(crate) struct Console<'a> {
    tty: OwnedTty,
    control: &'a RequestControl<'a>,
}

/// The terminal mode saved before hidden input. `restore` reports failure.
/// Drop restores only on a path that skipped `restore`, such as a panic.
struct ModeGuard {
    fd: RawFd,
    saved: libc::termios,
    restored: bool,
}

impl ModeGuard {
    fn restore(mut self) -> io::Result<()> {
        self.restored = true;
        // SAFETY: restores a termios read from this same, still open descriptor.
        if unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for ModeGuard {
    fn drop(&mut self) {
        if !self.restored {
            // SAFETY: restores a termios read from this same, still open descriptor.
            unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.saved) };
        }
    }
}

impl<'a> Console<'a> {
    /// Wrap the daemon's own non-blocking open of a checked terminal.
    pub(crate) fn new(fd: OwnedFd, control: &'a RequestControl<'a>) -> Self {
        Self {
            tty: OwnedTty::new(fd),
            control,
        }
    }

    pub(crate) fn fd(&self) -> BorrowedFd<'_> {
        self.tty.as_fd()
    }

    /// Write daemon text. Names from the vault are escaped first.
    pub(crate) fn say(&mut self, text: &str) -> Result<(), Interrupt> {
        self.tty.write_all(text.as_bytes(), self.control)
    }

    /// Fixed text that puts the terminal back. It runs inside the cleanup
    /// budget even after the request stopped.
    fn cleanup(&mut self, text: &str) -> Result<(), Interrupt> {
        let until = self.control.cleanup_deadline();
        self.tty.write_cleanup(text.as_bytes(), until)
    }

    /// Read one line with echo, line editing, and signals off. Bytes are kept
    /// in a zeroizing buffer that never grows past `limit`, so it never copies.
    /// A mode that could not be restored fails the read, so nothing it gated
    /// can run.
    pub(crate) fn read_hidden(
        &mut self,
        prompt: &str,
        limit: usize,
    ) -> Result<Zeroizing<Vec<u8>>, Interrupt> {
        let mode = self.hidden_mode().map_err(|_| Interrupt::Io)?;
        let mut line = Zeroizing::new(Vec::with_capacity(limit));
        let result = self.prompt_and_collect(prompt, &mut line, limit);
        let ended = self.cleanup("\r\n");
        let restored = mode.restore().map_err(|_| Interrupt::Io);
        result?;
        restored?;
        ended?;
        Ok(line)
    }

    fn prompt_and_collect(
        &mut self,
        prompt: &str,
        line: &mut Vec<u8>,
        limit: usize,
    ) -> Result<(), Interrupt> {
        // SAFETY: discards typeahead queued before the prompt appeared.
        if unsafe { libc::tcflush(self.tty.as_fd().as_raw_fd(), libc::TCIFLUSH) } != 0 {
            return Err(Interrupt::Io);
        }
        self.say(prompt)?;
        self.collect(line, limit)
    }

    fn collect(&mut self, line: &mut Vec<u8>, limit: usize) -> Result<(), Interrupt> {
        let mut chunk = Zeroizing::new([0u8; 256]);
        loop {
            let read = self.tty.read(&mut chunk[..], self.control)?;
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
        Ok(ModeGuard {
            fd,
            saved,
            restored: false,
        })
    }

    /// Show a value on its own screen, wait for Enter, then clear it, so the
    /// value never stays in scrollback. The clear runs on every path, inside
    /// the cleanup budget. A clear that fails is reported, never assumed.
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
        let cleared = self.cleanup("\x1b[2J\x1b[?1049l");
        shown.and(cleared)
    }

    /// Send a bounded OSC 52 clipboard write. The terminal cannot confirm it.
    /// A write cut short is cancelled with CAN, so a truncated value is
    /// never completed into the clipboard.
    pub(crate) fn osc52(&mut self, value: &[u8]) -> Result<(), Interrupt> {
        if value.len() > MAX_OSC52_VALUE {
            return Err(Interrupt::TooLong);
        }
        let mut sequence = Zeroizing::new(String::with_capacity(16 + value.len().div_ceil(3) * 4));
        sequence.push_str("\x1b]52;c;");
        base64::engine::general_purpose::STANDARD.encode_string(value, &mut sequence);
        sequence.push('\x07');
        if let Err(stop) = self.say(&sequence) {
            self.cleanup("\x18")?;
            return Err(stop);
        }
        Ok(())
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
