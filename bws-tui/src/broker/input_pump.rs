//! Reads a passed value input without letting it decide whether the daemon
//! blocks. A passed pipe or file shares its open file description, and with
//! it O_NONBLOCK, with the client and with every other copy. The daemon never
//! reads it or changes its flags. A forked child does the blocking read and
//! forwards the bytes through a pipe the daemon created and holds alone. The
//! daemon can always stop waiting and kill the child.
//!
//! After `fork` the child runs only raw async-signal-safe calls on memory
//! prepared before the fork. It never allocates, builds an error, panics,
//! unwinds, or returns into ordinary Rust.

use super::children::{exited_cleanly, Children};
use super::fields::MAX_VALUE;
use super::request::{Interrupt, RequestControl, CLEANUP_BUDGET};
use super::spawn_fd::{close_all_from, FdCeiling};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;
use zeroize::Zeroizing;

/// The child forwards at most one byte past the limit, so a long value is
/// recognised without reading all of it.
const FORWARD_LIMIT: usize = MAX_VALUE + 1;
const CHUNK: usize = 8192;
/// The child's own alarm outlives the request by this much, so it ends even
/// if the daemon dies first.
const ALARM_SLACK: Duration = Duration::from_secs(5);

/// Why the value input was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputStop {
    Stopped(Interrupt),
    TooLong,
    /// Every reader slot holds a child, possibly one that will not exit.
    Busy,
    Unreadable,
}

/// Read `input` to end of file, at most [`MAX_VALUE`] bytes, inside the request.
pub(crate) fn read_input(
    input: OwnedFd,
    control: &RequestControl<'_>,
    children: &Children,
    ceiling: FdCeiling,
) -> Result<Zeroizing<Vec<u8>>, InputStop> {
    control.check().map_err(InputStop::Stopped)?;
    let reservation = children.reserve().ok_or(InputStop::Busy)?;
    let (from_child, to_parent) = pipe().map_err(|_| InputStop::Unreadable)?;
    let mut buffer = Zeroizing::new(vec![0u8; CHUNK]);
    let alarm = control.remaining().unwrap_or(Duration::ZERO) + ALARM_SLACK;
    let plan = Plan::new(&input, &to_parent, ceiling, &mut buffer, alarm);
    let mut child = reservation.started(plan.fork().ok_or(InputStop::Unreadable)?);
    drop((input, to_parent));
    let read = drain(from_child, control);
    if read.is_err() {
        child.kill();
        child.wait_until(control.cleanup_deadline());
        return read;
    }
    let status = child.wait_until(control.deadline(CLEANUP_BUDGET));
    let value = read?;
    control.check().map_err(InputStop::Stopped)?;
    match status {
        Some(status) if exited_cleanly(status) && value.len() <= MAX_VALUE => Ok(value),
        _ if value.len() > MAX_VALUE => Err(InputStop::TooLong),
        _ => Err(InputStop::Unreadable),
    }
}

/// Read the daemon's own pipe to end of file into a buffer reserved once.
fn drain(from_child: File, control: &RequestControl<'_>) -> Result<Zeroizing<Vec<u8>>, InputStop> {
    let mut value = Zeroizing::new(Vec::with_capacity(FORWARD_LIMIT));
    let mut chunk = Zeroizing::new([0u8; CHUNK]);
    let mut reader = &from_child;
    loop {
        control.check().map_err(InputStop::Stopped)?;
        let room = (FORWARD_LIMIT - value.len()).min(CHUNK);
        match reader.read(&mut chunk[..room.max(1)]) {
            Ok(0) => return Ok(value),
            Ok(_) if room == 0 => return Err(InputStop::TooLong),
            Ok(read) => value.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                control
                    .wait(from_child.as_fd(), libc::POLLIN)
                    .map_err(InputStop::Stopped)?;
            }
            Err(_) => return Err(InputStop::Unreadable),
        }
    }
}

/// The daemon's read end is non-blocking. The child's write end blocks.
fn pipe() -> std::io::Result<(File, OwnedFd)> {
    let mut fds = [0i32; 2];
    // SAFETY: pipe writes two new descriptors into a valid array.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: both descriptors are new and owned only here.
    let (read, write) = unsafe { (File::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [read.as_raw_fd(), write.as_raw_fd()] {
        // SAFETY: sets close-on-exec on a descriptor owned here.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    // SAFETY: sets O_NONBLOCK on the read end this daemon alone holds.
    if unsafe { libc::fcntl(read.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((read, write))
}

/// Child exit codes. Only 0 means the input ended normally.
const EXIT_TOO_LONG: i32 = 3;
const EXIT_READ: i32 = 4;
const EXIT_WRITE: i32 = 5;
const EXIT_SETUP: i32 = 6;

/// Everything the child touches, prepared before `fork`.
struct Plan {
    input: libc::c_int,
    out: libc::c_int,
    ceiling: libc::c_int,
    buffer: *mut u8,
    /// Linux checks it after arming the parent-death signal. macOS relies on the alarm.
    #[cfg(target_os = "linux")]
    parent: libc::pid_t,
    alarm: libc::c_uint,
    errno: *mut libc::c_int,
    default_action: libc::sigaction,
    all: libc::sigset_t,
    none: libc::sigset_t,
}

impl Plan {
    fn new(
        input: &OwnedFd,
        out: &OwnedFd,
        ceiling: FdCeiling,
        buffer: &mut [u8],
        alarm: Duration,
    ) -> Self {
        // SAFETY: zeroed sigaction and sigset_t are valid; the calls fill them.
        let (mut default_action, mut all, mut none) = unsafe {
            (
                std::mem::zeroed::<libc::sigaction>(),
                std::mem::zeroed::<libc::sigset_t>(),
                std::mem::zeroed::<libc::sigset_t>(),
            )
        };
        default_action.sa_sigaction = libc::SIG_DFL;
        // SAFETY: each call writes one valid signal set.
        unsafe {
            libc::sigemptyset(&mut default_action.sa_mask);
            libc::sigfillset(&mut all);
            libc::sigemptyset(&mut none);
        }
        debug_assert_eq!(buffer.len(), CHUNK);
        Self {
            input: input.as_raw_fd(),
            out: out.as_raw_fd(),
            ceiling: ceiling.get(),
            buffer: buffer.as_mut_ptr(),
            // SAFETY: getpid cannot fail.
            #[cfg(target_os = "linux")]
            parent: unsafe { libc::getpid() },
            alarm: alarm.as_secs().clamp(1, u64::from(u32::MAX)) as libc::c_uint,
            errno: errno_location(),
            default_action,
            all,
            none,
        }
    }

    /// Fork with every signal blocked, so no handler runs in the child before
    /// it resets them. The parent's mask comes back before this returns.
    fn fork(&self) -> Option<libc::pid_t> {
        // SAFETY: a zeroed sigset_t is a valid out buffer.
        let mut saved: libc::sigset_t = unsafe { std::mem::zeroed() };
        // SAFETY: masks only this thread, then restores it below.
        if unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &self.all, &mut saved) } != 0 {
            return None;
        }
        // SAFETY: the child runs only `child`, which never returns.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // SAFETY: we are the forked child; the plan points at prepared memory.
            unsafe { self.child() }
        }
        // SAFETY: restores the mask saved above.
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &saved, std::ptr::null_mut()) };
        (pid > 0).then_some(pid)
    }

    unsafe fn child(&self) -> ! {
        let last_signal = if cfg!(target_os = "linux") { 65 } else { 32 };
        for signal in 1..last_signal {
            libc::sigaction(signal, &self.default_action, std::ptr::null_mut());
        }
        #[cfg(target_os = "linux")]
        {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            if libc::getppid() != self.parent {
                libc::_exit(EXIT_SETUP);
            }
        }
        libc::alarm(self.alarm);
        libc::pthread_sigmask(libc::SIG_SETMASK, &self.none, std::ptr::null_mut());
        let input = libc::fcntl(self.input, libc::F_DUPFD, 2);
        let out = libc::fcntl(self.out, libc::F_DUPFD, 2);
        if input < 0 || out < 0 || libc::dup2(input, 0) != 0 || libc::dup2(out, 1) != 1 {
            libc::_exit(EXIT_SETUP);
        }
        close_all_from(2, self.ceiling);
        self.copy()
    }

    /// Forward the input to the daemon's pipe until end of file.
    unsafe fn copy(&self) -> ! {
        let mut total = 0usize;
        loop {
            let want = (FORWARD_LIMIT - total).min(CHUNK);
            if want == 0 {
                self.exit(EXIT_TOO_LONG);
            }
            let read = libc::read(0, self.buffer.cast(), want);
            if read == 0 {
                self.exit(0);
            }
            if read < 0 {
                match *self.errno {
                    libc::EINTR => continue,
                    libc::EAGAIN => {
                        // The client made its description non-blocking. Wait instead.
                        let mut ready = libc::pollfd {
                            fd: 0,
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        libc::poll(&mut ready, 1, -1);
                        continue;
                    }
                    _ => self.exit(EXIT_READ),
                }
            }
            let read = read as usize;
            let mut sent = 0usize;
            while sent < read {
                let wrote = libc::write(1, self.buffer.add(sent).cast(), read - sent);
                if wrote < 0 && *self.errno == libc::EINTR {
                    continue;
                }
                if wrote <= 0 {
                    self.exit(EXIT_WRITE);
                }
                sent += wrote as usize;
            }
            total += read;
        }
    }

    /// Wipe the child's copy of the buffer, then end without running Rust exit code.
    unsafe fn exit(&self, code: i32) -> ! {
        for index in 0..CHUNK {
            std::ptr::write_volatile(self.buffer.add(index), 0);
        }
        libc::_exit(code)
    }
}

#[cfg(target_os = "linux")]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: returns this thread's errno slot, which the forked child keeps.
    unsafe { libc::__errno_location() }
}

#[cfg(target_os = "macos")]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: returns this thread's errno slot, which the forked child keeps.
    unsafe { libc::__error() }
}
