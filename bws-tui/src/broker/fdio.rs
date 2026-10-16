//! I/O on the daemon's own open of a checked terminal. The daemon opened it
//! non-blocking and no other process or worker shares that open, so no one
//! else can make a read or write block. Every attempt checks the request
//! first, so ready input or a fast write never skips a cancellation.

use super::request::{poll_millis, Interrupt, RequestControl};
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::time::Instant;

pub(crate) struct OwnedTty {
    file: File,
}

impl OwnedTty {
    /// `fd` must be a non-blocking open this daemon made and holds alone.
    pub(crate) fn new(fd: OwnedFd) -> Self {
        Self {
            file: File::from(fd),
        }
    }

    /// Read what is available inside the request. Zero means end of file.
    pub(crate) fn read(
        &mut self,
        buf: &mut [u8],
        control: &RequestControl<'_>,
    ) -> Result<usize, Interrupt> {
        let mut hung_up = false;
        loop {
            control.check()?;
            match self.file.read(buf) {
                Ok(read) => return control.check().map(|()| read),
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if hung_up {
                        return Err(Interrupt::Cancelled);
                    }
                    hung_up = control.wait(self.file.as_fd(), libc::POLLIN)?;
                }
                Err(_) => return Err(Interrupt::Io),
            }
        }
    }

    /// Write every byte inside the request.
    pub(crate) fn write_all(
        &mut self,
        bytes: &[u8],
        control: &RequestControl<'_>,
    ) -> Result<(), Interrupt> {
        self.write_with(
            bytes,
            |fd, events| control.wait(fd, events),
            || control.check(),
        )
    }

    /// Write fixed cleanup text by `until`, even after the request stopped.
    /// It waits on the terminal alone and never on the client.
    pub(crate) fn write_cleanup(&mut self, bytes: &[u8], until: Instant) -> Result<(), Interrupt> {
        self.write_with(bytes, |fd, events| wait_alone(fd, events, until), || Ok(()))
    }

    fn write_with(
        &mut self,
        mut bytes: &[u8],
        mut wait: impl FnMut(BorrowedFd<'_>, libc::c_short) -> Result<bool, Interrupt>,
        check: impl Fn() -> Result<(), Interrupt>,
    ) -> Result<(), Interrupt> {
        let mut hung_up = false;
        while !bytes.is_empty() {
            check()?;
            match self.file.write(bytes) {
                Ok(0) => return Err(Interrupt::Io),
                Ok(written) => {
                    bytes = &bytes[written..];
                    hung_up = false;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if hung_up {
                        return Err(Interrupt::Io);
                    }
                    hung_up = wait(self.file.as_fd(), libc::POLLOUT)?;
                }
                Err(_) => return Err(Interrupt::Io),
            }
        }
        Ok(())
    }
}

impl AsFd for OwnedTty {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

/// Wait on `fd` alone until `until`. True means only a hangup or error.
fn wait_alone(
    fd: BorrowedFd<'_>,
    events: libc::c_short,
    until: Instant,
) -> Result<bool, Interrupt> {
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Interrupt::TimedOut);
        }
        let mut ready = libc::pollfd {
            fd: fd.as_raw_fd(),
            events,
            revents: 0,
        };
        // SAFETY: one valid pollfd entry.
        if unsafe { libc::poll(&mut ready, 1, poll_millis(left)) } < 0 {
            if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(Interrupt::Io);
        }
        if ready.revents & libc::POLLNVAL != 0 {
            return Err(Interrupt::Io);
        }
        if ready.revents & events != 0 {
            return Ok(false);
        }
        if ready.revents != 0 {
            return Ok(true);
        }
    }
}
