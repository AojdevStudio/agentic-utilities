//! `hush run` (and its alias `hush exec`): the broker daemon starts the child
//! with the named secrets, and this client relays masked output. The client
//! never fetches a value; without a reachable daemon, the run fails.

use crate::broker::meta::SecretRef;
use crate::broker::runspec::{
    is_passthrough_allowed, Binding, Delivery, EnvName, ForwardSignal, OutputSink, RunOutcome,
    RunSpec,
};
use crate::broker::{default_socket_path, Client, ConnectOutcome, RunInputs};
use anyhow::{anyhow, bail, Result};
use std::ffi::OsString;
use std::io::Write;
use std::sync::atomic::{AtomicI32, Ordering};

pub(crate) fn parse_binding(
    raw: &str,
    flag: &str,
    delivery: Delivery,
    project: Option<&str>,
) -> Result<Binding> {
    let (key, env) = raw.split_once('=').unwrap_or((raw, raw));
    if key.is_empty() || env.is_empty() {
        bail!("invalid {flag} '{raw}': expected KEY or KEY=ENVNAME");
    }
    let env = EnvName::try_from(env.to_owned()).map_err(|_| {
        anyhow!("'{env}' is not a valid env var name. Remap it with {flag} '{key}=ENVNAME'")
    })?;
    Ok(Binding {
        secret: SecretRef::Key {
            key: key.to_owned(),
            project: project.map(ToOwned::to_owned),
        },
        env,
        delivery,
    })
}

pub(crate) fn build_spec(bindings: Vec<Binding>, cmd: Vec<OsString>) -> Result<RunSpec> {
    let mut cmd = cmd.into_iter();
    let program = cmd
        .next()
        .ok_or_else(|| anyhow!("no command given after --"))?;
    let env_passthrough = std::env::vars_os()
        .filter_map(|(name, value)| {
            let name = name
                .into_string()
                .ok()
                .filter(|name| is_passthrough_allowed(name))?;
            Some((EnvName::try_from(name).ok()?, value))
        })
        .collect();
    Ok(RunSpec {
        bindings,
        program,
        args: cmd.collect(),
        cwd: std::env::current_dir()?,
        env_passthrough,
    })
}

/// `hush run` and `hush exec` share this path. Each flag list parses with its own name.
pub(crate) fn run_flags(
    flags: &[(&Vec<String>, &str, Delivery)],
    project: Option<String>,
    no_stdin: bool,
    cmd: Vec<OsString>,
) -> Result<()> {
    let mut bindings = Vec::new();
    for (raws, flag, delivery) in flags {
        for raw in raws.iter() {
            bindings.push(parse_binding(raw, flag, *delivery, project.as_deref())?);
        }
    }
    run(bindings, cmd, no_stdin)
}

/// Run through the daemon and exit with the child's status. Never returns on success.
/// With `no_stdin`, the caller's stdin is never read and the child sees end of input.
pub(crate) fn run(bindings: Vec<Binding>, cmd: Vec<OsString>, no_stdin: bool) -> Result<()> {
    let spec = build_spec(bindings, cmd)?;
    let mut client = match Client::try_connect()? {
        ConnectOutcome::Connected(client) => client,
        ConnectOutcome::Unavailable => bail!(
            "hush run requires the hush broker daemon, and none is reachable at {}. \
hush never fetches secret values in the client. Start the daemon (see docs/adr/0002-hush-broker.md)",
            default_socket_path().display()
        ),
    };
    let inputs = RunInputs {
        stdin: (!no_stdin).then(|| Box::new(std::io::stdin()) as Box<dyn std::io::Read + Send>),
        signals: Some(forward_signals()?),
    };
    match client.run_with(spec, &mut StdSink, inputs)? {
        RunOutcome::Exited(code) => std::process::exit(code),
        RunOutcome::Signaled(signal) => {
            // Die the same way the child did, so a parent shell sees the signal.
            // SAFETY: resetting one disposition and raising a signal at ourselves.
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                libc::raise(signal);
            }
            std::process::exit(128 + signal)
        }
    }
}

struct StdSink;

impl OutputSink for StdSink {
    fn stdout(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut out = std::io::stdout().lock();
        out.write_all(bytes)?;
        out.flush()
    }

    fn stderr(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut err = std::io::stderr().lock();
        err.write_all(bytes)?;
        err.flush()
    }

    fn notice(&mut self, message: &str) -> std::io::Result<()> {
        writeln!(std::io::stderr().lock(), "{message}")
    }
}

static SIGNAL_PIPE: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(signal: libc::c_int) {
    let byte = signal as u8;
    // SAFETY: write is async-signal-safe and the fd stays open for the process lifetime.
    unsafe {
        libc::write(
            SIGNAL_PIPE.load(Ordering::Relaxed),
            (&byte as *const u8).cast(),
            1,
        )
    };
}

fn forward_signals() -> Result<std::sync::mpsc::Receiver<ForwardSignal>> {
    let mut fds = [0i32; 2];
    // SAFETY: pipe writes two fds into a valid array.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    SIGNAL_PIPE.store(fds[1], Ordering::Relaxed);
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
        // SAFETY: the handler only calls write on a pipe fd.
        let previous = unsafe { libc::signal(signal, on_signal as *const () as usize) };
        if previous == libc::SIG_ERR {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let read_fd = fds[0];
    std::thread::spawn(move || loop {
        let mut byte = 0u8;
        // SAFETY: reading one byte into a valid buffer from our own pipe.
        let read = unsafe { libc::read(read_fd, (&mut byte as *mut u8).cast(), 1) };
        if read == 1 {
            if let Some(signal) = ForwardSignal::from_number(i32::from(byte)) {
                if tx.send(signal).is_err() {
                    return;
                }
            }
        } else if read == 0
            || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            return;
        }
    });
    Ok(rx)
}
