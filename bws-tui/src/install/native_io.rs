//! One native child's token input, output and lifetime under one budget.

use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::{Read, Write};
use std::process::Child;
use std::time::{Duration, Instant};

/// Time and output limits for one native command.
#[derive(Debug, Clone, Copy)]
pub(super) struct Budget {
    pub time: Duration,
    pub output: u64,
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

/// Feed the token, collect bounded stdout and wait for the child. A child
/// past the deadline is killed and reaped.
pub(super) fn drive(
    program: &str,
    mut child: Child,
    input: Option<&SecretString>,
    budget: Budget,
) -> Result<(Option<i32>, String)> {
    if let Some(token) = input {
        let mut pipe = child.stdin.take().context("no stdin pipe")?;
        #[cfg(all(test, target_os = "linux"))]
        if let Some(size) = budget.input_pipe {
            use std::os::fd::AsRawFd;
            // SAFETY: F_SETPIPE_SZ resizes this process's own open pipe end.
            assert!(unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETPIPE_SZ, size) } > 0);
        }
        let written = pipe.write_all(token.expose_secret().as_bytes());
        drop(pipe);
        if written.is_err() {
            reap(&mut child);
            bail!("{program} did not accept its input");
        }
    }
    let limit = budget.output;
    let reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout.take(limit).read_to_string(&mut text);
            text
        })
    });
    let deadline = Instant::now() + budget.time;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() > deadline {
            reap(&mut child);
            bail!(
                "{program} did not finish within {} seconds",
                budget.time.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let text = reader
        .map(|reader| reader.join().unwrap_or_default())
        .unwrap_or_default();
    Ok((status.code(), text))
}

fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
