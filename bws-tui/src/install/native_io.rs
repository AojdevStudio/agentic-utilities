//! One native child's token input, output and lifetime under one deadline.
//! The deadline starts before spawn. Both pipes are nonblocking and polled on
//! this thread, so no writer or reader outlives the call. On any failure the
//! child's process group is killed and the child is reaped.

use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::process::Child;
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
    let exchanged = exchange(&mut child, input, deadline, budget).and_then(|output| {
        let status = child.wait().context("cannot reap the child")?;
        Ok((status.code(), String::from_utf8(output).ok()))
    });
    match exchanged {
        Ok((code, Some(text))) => Ok((code, text)),
        Ok((_, None)) => bail!("{program} printed output that is not UTF-8"),
        Err(error) => {
            let error = error.context(format!("{program} was stopped"));
            Err(match stop(&mut child) {
                Ok(()) => error,
                Err(reap) => error.context(format!("{program} could not be reaped ({reap})")),
            })
        }
    }
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
        #[cfg(all(test, target_os = "linux"))]
        if let Some(size) = budget.input_pipe {
            // SAFETY: F_SETPIPE_SZ resizes this process's own open pipe end.
            assert!(unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETPIPE_SZ, size) } > 0);
        }
        nonblocking(pipe.as_fd())?;
    }
    if let Some(pipe) = &stdout {
        nonblocking(pipe.as_fd())?;
    }
    let (mut written, mut output, mut chunk) = (0, Vec::new(), [0u8; 8192]);
    loop {
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
