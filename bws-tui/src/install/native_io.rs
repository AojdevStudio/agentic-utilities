//! One native child's token input, output and lifetime under one deadline.
//! The deadline starts before spawn. Both pipes are nonblocking and polled on
//! this thread, so no writer or reader outlives the call. On any failure the
//! child's process group is killed and the child is reaped. The child's own
//! group misses a terminal's stop signal, so that signal is held while a
//! child is owned and delivered once its group is stopped and it is reaped.

use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::process::Child;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering::SeqCst};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Time and output limits for one native command.
#[derive(Debug, Clone, Copy)]
pub(super) struct Budget {
    pub time: Duration,
    pub output: usize,
    /// Test only: shrink the child's stdin pipe to model a small Linux pipe.
    #[cfg(test)]
    pub input_pipe: Option<libc::c_int>,
}

pub(super) const BUDGET: Budget = Budget {
    time: Duration::from_secs(120),
    output: 1 << 20,
    #[cfg(test)]
    input_pipe: None,
};

/// How often a quiet child is checked for exit.
const TICK: Duration = Duration::from_millis(10);

/// Signals a terminal or a supervisor sends to end the installer.
const STOP_SIGNALS: [libc::c_int; 4] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT];
/// Native children this process owns now.
static OWNED: AtomicUsize = AtomicUsize::new(0);
/// A stop signal held until no owned child runs, or 0.
static HELD: AtomicI32 = AtomicI32::new(0);

/// Cleanup ownership of one native child. Take it before spawn, so a signal
/// at startup is held too, and drop it after the reap.
pub(super) struct Owned(());

impl Owned {
    pub(super) fn take() -> Result<Self> {
        static HANDLERS: OnceLock<io::Result<()>> = OnceLock::new();
        if let Err(error) = HANDLERS.get_or_init(hold_stop_signals) {
            bail!("cannot arm interrupt cleanup ({error})");
        }
        OWNED.fetch_add(1, SeqCst);
        Ok(Self(()))
    }
}

impl Drop for Owned {
    /// The last owner ends the process with a held signal's default action.
    fn drop(&mut self) {
        if OWNED.fetch_sub(1, SeqCst) == 1 {
            let signal = HELD.load(SeqCst);
            if signal != 0 {
                die(signal);
                std::process::exit(128 + signal);
            }
        }
    }
}

/// Feed the token, collect stdout and wait for the child, all before
/// `deadline`. Output past the budget or not UTF-8 is an error, never a
/// partial result. The child must lead its own process group.
pub(super) fn drive(
    program: &str,
    mut child: Child,
    input: Option<&SecretString>,
    deadline: Instant,
    budget: Budget,
) -> Result<(Option<i32>, String)> {
    // Output is checked before the reap, so a failure still owns the group.
    let exchanged = exchange(&mut child, input, deadline, budget).and_then(|output| {
        let text = String::from_utf8(output).map_err(|_| anyhow!("the output is not UTF-8"))?;
        let status = child.wait().context("cannot reap the child")?;
        Ok((status.code(), text))
    });
    exchanged.map_err(|error| {
        let error = error.context(format!("{program} was stopped"));
        match stop(&mut child) {
            Ok(()) => error,
            Err(reap) => error.context(format!("{program} could not be reaped ({reap})")),
        }
    })
}

/// Move bytes until the token is written, stdout is closed and the child has
/// exited. The child stays unreaped, so its pid and process group stay ours.
fn exchange(
    child: &mut Child,
    input: Option<&SecretString>,
    deadline: Instant,
    budget: Budget,
) -> Result<Vec<u8>> {
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let token = input.map_or(&[][..], |token| token.expose_secret().as_bytes());
    if let Some(pipe) = &stdin {
        #[cfg(test)]
        shrink(pipe.as_fd(), budget.input_pipe);
        nonblocking(pipe.as_fd())?;
    }
    if let Some(pipe) = &stdout {
        nonblocking(pipe.as_fd())?;
    }
    let (mut written, mut output, mut chunk) = (0, Vec::new(), [0u8; 8192]);
    loop {
        if HELD.load(SeqCst) != 0 {
            bail!("a stop signal arrived");
        }
        if written == token.len() {
            // Closing stdin is how the helper learns the token is complete.
            stdin = None;
        }
        let exited = exited(child)?;
        if exited && stdin.is_none() && stdout.is_none() {
            return Ok(output);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            bail!("the deadline passed");
        }
        let mut fds: Vec<libc::pollfd> = [
            stdin
                .as_ref()
                .map(|pipe| poll_for(pipe.as_fd(), libc::POLLOUT)),
            stdout
                .as_ref()
                .map(|pipe| poll_for(pipe.as_fd(), libc::POLLIN)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let wait = left.min(TICK).as_millis() as libc::c_int;
        // SAFETY: `fds` is a live array of `fds.len()` pollfd entries.
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, wait) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != ErrorKind::Interrupted {
                return Err(anyhow!(error).context("cannot poll the child's pipes"));
            }
        }
        if let Some(pipe) = stdin.as_mut() {
            match pipe.write(&token[written..]) {
                Ok(count) => written += count,
                Err(error) if pending(&error) => {}
                Err(_) => bail!("the input was not accepted"),
            }
        }
        if let Some(pipe) = stdout.as_mut() {
            match pipe.read(&mut chunk) {
                Ok(0) => stdout = None,
                Ok(count) if output.len() + count > budget.output => {
                    bail!("the output exceeded {} bytes", budget.output)
                }
                Ok(count) => output.extend_from_slice(&chunk[..count]),
                Err(error) if pending(&error) => {}
                Err(error) => return Err(anyhow!(error).context("cannot read the output")),
            }
        }
    }
}

/// Whether the child has exited, without reaping it.
fn exited(child: &Child) -> Result<bool> {
    // SAFETY: zeroed siginfo_t is a valid out buffer. WNOWAIT leaves the
    // child waitable, so its pid cannot be reused before the final wait.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
    if unsafe { libc::waitid(libc::P_PID, child.id(), &mut info, flags) } != 0 {
        return Err(anyhow!(io::Error::last_os_error()).context("cannot wait for the child"));
    }
    #[cfg(target_os = "linux")]
    // SAFETY: waitid filled a child state, or left the zeroed pid.
    let pid = unsafe { info.si_pid() };
    #[cfg(target_os = "macos")]
    let pid = info.si_pid;
    Ok(pid != 0)
}

/// Kill the child's whole process group, then reap the child. The group id
/// is the unreaped child's pid, so the signal reaches only its descendants.
fn stop(child: &mut Child) -> io::Result<()> {
    // SAFETY: the child leads its own group and is not yet reaped.
    unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    child.wait().map(drop)
}

/// Install the holding handler once, keeping a signal the caller ignores ignored.
fn hold_stop_signals() -> io::Result<()> {
    for signal in STOP_SIGNALS {
        // SAFETY: zeroed sigaction values are valid buffers, and each call
        // names one valid signal.
        unsafe {
            let mut current: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(signal, std::ptr::null(), &mut current) != 0 {
                return Err(io::Error::last_os_error());
            }
            if current.sa_sigaction == libc::SIG_IGN {
                continue;
            }
            let mut hold: libc::sigaction = std::mem::zeroed();
            hold.sa_sigaction = on_stop as *const () as libc::sighandler_t;
            hold.sa_flags = libc::SA_RESTART;
            libc::sigfillset(&mut hold.sa_mask);
            if libc::sigaction(signal, &hold, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

/// Hold the signal while a child is owned, otherwise die from it now. The
/// store comes before the load, and an owner drops before it loads, so one
/// of them always sees the other.
extern "C" fn on_stop(signal: libc::c_int) {
    HELD.store(signal, SeqCst);
    if OWNED.load(SeqCst) == 0 {
        die(signal);
    }
}

/// End the process as `signal`'s default action does. Inside the handler the
/// signal stays blocked until it returns, then it is delivered.
fn die(signal: libc::c_int) {
    // SAFETY: signal and raise are async-signal-safe and name a valid signal.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        libc::raise(signal);
    }
}

/// Test only: model a small Linux pipe. macOS has no per-pipe size control,
/// so its tests keep the native size.
#[cfg(test)]
fn shrink(fd: BorrowedFd<'_>, size: Option<libc::c_int>) {
    #[cfg(target_os = "linux")]
    if let Some(size) = size {
        // SAFETY: F_SETPIPE_SZ resizes this process's own open pipe end.
        assert!(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETPIPE_SZ, size) } > 0);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (fd, size);
}

fn nonblocking(fd: BorrowedFd<'_>) -> Result<()> {
    // SAFETY: F_GETFL and F_SETFL on this process's own open descriptor.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(anyhow!(io::Error::last_os_error()).context("cannot set up the child's pipes"));
    }
    Ok(())
}

fn poll_for(fd: BorrowedFd<'_>, events: libc::c_short) -> libc::pollfd {
    libc::pollfd {
        fd: fd.as_raw_fd(),
        events,
        revents: 0,
    }
}

fn pending(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted)
}
