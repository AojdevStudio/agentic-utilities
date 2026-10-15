//! One `bws` call as a daemon-owned child. Its slot is reserved before spawn
//! and released only once the child is reaped. Its stdout is a pipe the
//! daemon reads without blocking, capped in size, and bounded by the call
//! limit, the request deadline, and the client. Nothing outlives the call:
//! no reader thread and no blocking wait.

use super::children::{exited_cleanly, Children, OwnedChild};
use super::request::{Interrupt, RequestControl};
use anyhow::{anyhow, Context};
use secrecy::SecretString;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// The only PATH a `bws` child sees.
pub(super) const BWS_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// A `bws` invocation. Values never appear in `args`.
pub(super) struct BwsCall<'a> {
    pub path: &'a Path,
    pub token: &'a SecretString,
    pub args: &'a [&'a str],
}

pub(super) enum BwsError {
    /// Proven never to have run: no slot, or the spawn itself failed.
    NotStarted(anyhow::Error),
    /// It ran and did not finish cleanly.
    Failed(anyhow::Error),
}

impl From<BwsError> for anyhow::Error {
    fn from(error: BwsError) -> Self {
        match error {
            BwsError::NotStarted(error) | BwsError::Failed(error) => error,
        }
    }
}

/// Run `call`, reading at most `cap` bytes of stdout within `limit`.
/// The output holds secret values, so it zeroizes on drop.
pub(super) fn run(
    call: &BwsCall<'_>,
    cap: usize,
    limit: Duration,
    control: &RequestControl<'_>,
    children: &Children,
) -> Result<Zeroizing<String>, BwsError> {
    let reservation = children
        .reserve()
        .ok_or_else(|| BwsError::NotStarted(anyhow!("too many bws calls are running")))?;
    let (pid, stdout) = super::bws_spawn::spawn(call)
        .map_err(|error| BwsError::NotStarted(anyhow!(error).context("failed to run bws")))?;
    let mut child = reservation.started(pid);
    let Some(stdout) = stdout else {
        child.stop(control.cleanup_deadline());
        return Err(BwsError::Failed(anyhow!(
            "bws started without a stdout pipe"
        )));
    };
    let until = control.deadline(limit);
    let read = read_capped(File::from(stdout), cap, control, until);
    let mut bytes = match read {
        Ok(bytes) => bytes,
        Err(error) => {
            child.stop(control.cleanup_deadline());
            return Err(BwsError::Failed(error));
        }
    };
    let status = reap(&mut child, control, until)?;
    if !exited_cleanly(status) {
        let name = call
            .args
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        return Err(BwsError::Failed(anyhow!(
            "bws {name} failed ({})",
            describe(status)
        )));
    }
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(text) => Ok(Zeroizing::new(text)),
        Err(error) => {
            error.into_bytes().zeroize();
            Err(BwsError::Failed(anyhow!("bws produced invalid UTF-8")))
        }
    }
}

fn read_capped(
    output: File,
    cap: usize,
    control: &RequestControl<'_>,
    until: Instant,
) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    // SAFETY: sets O_NONBLOCK on the read end of a pipe this daemon alone holds.
    if unsafe { libc::fcntl(output.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) } != 0 {
        return Err(std::io::Error::last_os_error()).context("failed to run bws");
    }
    let mut out = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        control.check().map_err(stopped)?;
        match (&output).read(&mut chunk[..]) {
            Ok(0) => return Ok(out),
            Ok(read) if out.len() + read > cap => {
                return Err(anyhow!("bws output exceeded the size limit"))
            }
            Ok(read) => out.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                control
                    .wait_until(output.as_fd(), libc::POLLIN, Some(until))
                    .map_err(stopped)?;
            }
            Err(error) => return Err(error).context("failed to run bws"),
        }
    }
}

/// End of output is not the end of the call. Keep watching the request until
/// the child is reaped. A child that already exited wins over a stop seen at
/// the same moment, so a finished call keeps its real result.
fn reap(
    child: &mut OwnedChild<'_>,
    control: &RequestControl<'_>,
    until: Instant,
) -> Result<libc::c_int, BwsError> {
    loop {
        if let Some(status) = child.try_reap() {
            return Ok(status);
        }
        let waited = if Instant::now() >= until {
            Err(Interrupt::TimedOut)
        } else {
            control.nap(Duration::from_millis(1))
        };
        if let Err(stop) = waited {
            if let Some(status) = child.try_reap() {
                return Ok(status);
            }
            child.stop(control.cleanup_deadline());
            return Err(BwsError::Failed(stopped(stop)));
        }
    }
}

fn stopped(interrupt: Interrupt) -> anyhow::Error {
    match interrupt {
        Interrupt::TimedOut => anyhow!("bws timed out"),
        Interrupt::Disconnected => anyhow!("bws call cancelled: the client left"),
        _ => anyhow!("failed to run bws"),
    }
}

fn describe(status: libc::c_int) -> String {
    if libc::WIFEXITED(status) {
        format!("exit status {}", libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        format!("signal {}", libc::WTERMSIG(status))
    } else {
        "unknown exit".into()
    }
}
