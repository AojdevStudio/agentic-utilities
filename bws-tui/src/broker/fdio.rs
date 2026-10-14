//! Bounded, cancellable I/O on descriptors a client passed in. A passed
//! descriptor runs in non-blocking mode while the daemon holds it, so a
//! paused terminal or another reader that wins a race cannot hang a worker.
//! Every wait also watches the client's socket. Activity there means the
//! client hung up or broke protocol, and the wait stops.

use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::time::Instant;

/// Why a read or write on a passed descriptor stopped early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interrupt {
    /// Ctrl-C, Ctrl-D on an empty line, or a terminal hangup.
    Cancelled,
    /// The client hung up or sent data mid-operation.
    Disconnected,
    TimedOut,
    TooLong,
    Io,
}

/// A passed descriptor in non-blocking mode. Drop restores the original
/// status flags, because the client shares the open file.
pub(crate) struct Passed {
    file: File,
    flags: libc::c_int,
}

impl Passed {
    pub(crate) fn new(fd: OwnedFd) -> io::Result<Self> {
        // SAFETY: F_GETFL only reads the status flags of an owned descriptor.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: F_SETFL changes only the status flags of an owned descriptor.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            file: File::from(fd),
            flags,
        })
    }

    /// Read what is available, waiting until `deadline`. Zero means end of file.
    pub(crate) fn read(
        &mut self,
        buf: &mut [u8],
        socket: &UnixStream,
        deadline: Instant,
    ) -> Result<usize, Interrupt> {
        let mut hung_up = false;
        loop {
            match self.file.read(buf) {
                Ok(read) => return Ok(read),
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if hung_up {
                        return Err(Interrupt::Cancelled);
                    }
                    hung_up = wait(self.file.as_fd(), libc::POLLIN, socket, deadline)?;
                }
                Err(_) => return Err(Interrupt::Io),
            }
        }
    }

    pub(crate) fn write_all(
        &mut self,
        mut bytes: &[u8],
        socket: &UnixStream,
        deadline: Instant,
    ) -> Result<(), Interrupt> {
        let mut hung_up = false;
        while !bytes.is_empty() {
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
                    hung_up = wait(self.file.as_fd(), libc::POLLOUT, socket, deadline)?;
                }
                Err(_) => return Err(Interrupt::Io),
            }
        }
        Ok(())
    }
}

impl AsFd for Passed {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

impl Drop for Passed {
    fn drop(&mut self) {
        // SAFETY: restores status flags read from this same, still open descriptor.
        unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_SETFL, self.flags) };
    }
}

/// True when `fd` reported only a hangup or error, so the next attempt must not wait again.
fn wait(
    fd: BorrowedFd<'_>,
    events: libc::c_short,
    socket: &UnixStream,
    deadline: Instant,
) -> Result<bool, Interrupt> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Interrupt::TimedOut);
        }
        let mut fds = [
            libc::pollfd {
                fd: fd.as_raw_fd(),
                events,
                revents: 0,
            },
            libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let millis = left.as_millis().clamp(1, i32::MAX as u128) as i32;
        // SAFETY: two valid pollfd entries.
        if unsafe { libc::poll(fds.as_mut_ptr(), 2, millis) } < 0 {
            if io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(Interrupt::Io);
        }
        let (ready, client) = (fds[0].revents, fds[1].revents);
        if client != 0 {
            return Err(Interrupt::Disconnected);
        }
        if ready & libc::POLLNVAL != 0 {
            return Err(Interrupt::Io);
        }
        if ready & events != 0 {
            return Ok(false);
        }
        if ready != 0 {
            return Ok(true);
        }
    }
}
