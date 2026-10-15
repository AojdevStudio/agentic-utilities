//! One request's clock and cancellation. A human request gets one absolute
//! deadline at handler entry, and nothing it does later renews it. Any
//! activity on the client's socket during a human request means the client
//! hung up or broke protocol, and that stop is sticky. Every wait checks both
//! before it starts and after it wakes, so ready bytes or a fast reply never
//! skip a check.

use super::meta::WriteFailure;
use std::cell::Cell;
use std::io::{self, ErrorKind};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

/// The single budget for putting a terminal back and stopping children once
/// a request stops. An engineering bound, not a measured one.
pub(crate) const CLEANUP_BUDGET: Duration = Duration::from_millis(250);

/// Why a read, write, or wait stopped early.
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

pub(crate) struct RequestControl<'a> {
    socket: Option<&'a UnixStream>,
    expires: Option<Instant>,
    stop: Cell<Option<Interrupt>>,
    cleanup: Cell<Option<Instant>>,
}

impl<'a> RequestControl<'a> {
    /// A human request on `socket` that must finish within `limit`.
    pub(crate) fn until(socket: &'a UnixStream, limit: Duration) -> Self {
        Self {
            socket: Some(socket),
            expires: Instant::now().checked_add(limit),
            stop: Cell::new(None),
            cleanup: Cell::new(None),
        }
    }

    /// A caller with no client to watch and no request deadline, such as a
    /// catalog read or run preparation. Each call keeps its own limit.
    pub(crate) fn detached() -> Self {
        Self {
            socket: None,
            expires: None,
            stop: Cell::new(None),
            cleanup: Cell::new(None),
        }
    }

    /// Fails once the client left or the deadline passed, and stays failed.
    pub(crate) fn check(&self) -> Result<(), Interrupt> {
        if let Some(stop) = self.stop.get() {
            return Err(stop);
        }
        if self
            .expires
            .is_some_and(|expires| Instant::now() >= expires)
        {
            return Err(self.stopped(Interrupt::TimedOut));
        }
        let Some(socket) = self.socket else {
            return Ok(());
        };
        loop {
            let mut fds = [pollfd(socket.as_fd(), libc::POLLIN)];
            // SAFETY: one valid pollfd entry, no wait.
            let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, 0) };
            if ready < 0 && io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                continue;
            }
            if ready != 0 {
                return Err(self.stopped(Interrupt::Disconnected));
            }
            return Ok(());
        }
    }

    /// The earlier of `limit` from now and the request deadline.
    pub(crate) fn deadline(&self, limit: Duration) -> Instant {
        let call = Instant::now() + limit;
        self.expires.map_or(call, |expires| expires.min(call))
    }

    /// Time left before the request deadline, if it has one.
    pub(crate) fn remaining(&self) -> Option<Duration> {
        self.expires
            .map(|expires| expires.saturating_duration_since(Instant::now()))
    }

    /// The client socket a waiter must watch, if any.
    pub(crate) fn socket(&self) -> Option<BorrowedFd<'a>> {
        self.socket.map(AsFd::as_fd)
    }

    /// Wait for `events` on `fd` until the request deadline. True means the
    /// descriptor reported only a hangup or error.
    pub(crate) fn wait(
        &self,
        fd: BorrowedFd<'_>,
        events: libc::c_short,
    ) -> Result<bool, Interrupt> {
        self.wait_until(fd, events, None)
    }

    /// Like [`RequestControl::wait`], also bounded by one call's own `until`.
    /// A passed call limit is a non-sticky `TimedOut`.
    pub(crate) fn wait_until(
        &self,
        fd: BorrowedFd<'_>,
        events: libc::c_short,
        until: Option<Instant>,
    ) -> Result<bool, Interrupt> {
        loop {
            self.check()?;
            let end = match (until, self.expires) {
                (Some(call), Some(expires)) => Some(call.min(expires)),
                (end, None) | (None, end) => end,
            };
            let millis = match end {
                Some(end) if Instant::now() >= end => return Err(Interrupt::TimedOut),
                Some(end) => poll_millis(end.saturating_duration_since(Instant::now())),
                None => -1,
            };
            let mut fds = [pollfd(fd, events), pollfd(fd, 0)];
            let count = match self.socket {
                Some(socket) => {
                    fds[1] = pollfd(socket.as_fd(), libc::POLLIN);
                    2
                }
                None => 1,
            };
            // SAFETY: `count` valid pollfd entries.
            if unsafe { libc::poll(fds.as_mut_ptr(), count, millis) } < 0 {
                if io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(Interrupt::Io);
            }
            if count == 2 && fds[1].revents != 0 {
                return Err(self.stopped(Interrupt::Disconnected));
            }
            let ready = fds[0].revents;
            if ready & libc::POLLNVAL != 0 {
                return Err(Interrupt::Io);
            }
            if ready & events != 0 {
                self.check()?;
                return Ok(false);
            }
            if ready != 0 {
                self.check()?;
                return Ok(true);
            }
        }
    }

    /// Sleep up to `pause`, waking early when the client leaves.
    pub(crate) fn nap(&self, pause: Duration) -> Result<(), Interrupt> {
        self.check()?;
        let pause = self.remaining().map_or(pause, |left| left.min(pause));
        match self.socket {
            Some(socket) => {
                let mut fds = [pollfd(socket.as_fd(), libc::POLLIN)];
                // SAFETY: one valid pollfd entry. EINTR only shortens the nap.
                unsafe { libc::poll(fds.as_mut_ptr(), 1, poll_millis(pause)) };
            }
            None => std::thread::sleep(pause),
        }
        self.check()
    }

    /// The one cleanup deadline. The first call fixes it for the request.
    pub(crate) fn cleanup_deadline(&self) -> Instant {
        let deadline = self
            .cleanup
            .get()
            .unwrap_or_else(|| Instant::now() + CLEANUP_BUDGET);
        self.cleanup.set(Some(deadline));
        deadline
    }

    fn stopped(&self, stop: Interrupt) -> Interrupt {
        self.stop.set(Some(stop));
        stop
    }
}

/// One write's admission. A stop before [`WriteAttempt::begin_remote`] means
/// nothing was sent. After it, the provider may hold the change.
pub(crate) struct WriteAttempt<'a> {
    control: &'a RequestControl<'a>,
}

impl<'a> WriteAttempt<'a> {
    pub(crate) fn new(control: &'a RequestControl<'a>) -> Self {
        Self { control }
    }

    pub(crate) fn control(&self) -> &'a RequestControl<'a> {
        self.control
    }

    /// The last check before a backend sends the write. Call it immediately
    /// before the first step that can reach the provider.
    pub(crate) fn begin_remote(&mut self) -> Result<(), WriteFailure> {
        self.control.check().map_err(|_| WriteFailure::Stopped)
    }
}

fn pollfd(fd: BorrowedFd<'_>, events: libc::c_short) -> libc::pollfd {
    libc::pollfd {
        fd: fd.as_raw_fd(),
        events,
        revents: 0,
    }
}

/// Round up, so a wait never ends a millisecond short and spins.
pub(crate) fn poll_millis(left: Duration) -> i32 {
    let millis = left.as_micros().div_ceil(1000);
    millis.clamp(0, i32::MAX as u128) as i32
}
