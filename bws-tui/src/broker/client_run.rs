//! `Client::run`. The client sends names and its own context, then relays
//! masked output. It never receives a secret value.

use super::client::Client;
use super::meta::SecretRef;
use super::protocol::{
    read_frame, write_frame, Frame, RequestFrame, ResponseFrame, WireBinding, WireBytes,
    WireRunSpec,
};
use super::runspec::{ForwardSignal, OutputSink, RunOutcome, RunSpec, StreamKind};
use anyhow::{anyhow, bail, Result};
use std::io::{ErrorKind, Read};
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Optional live inputs for a run. Without stdin the child sees end of file at once.
#[derive(Default)]
pub(crate) struct RunInputs {
    pub stdin: Option<Box<dyn Read + Send>>,
    pub signals: Option<Receiver<ForwardSignal>>,
}

impl Client {
    /// The ADR 0002 surface. The CLI uses `run_with` to attach stdin and signals.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn run(&mut self, spec: RunSpec, sink: &mut dyn OutputSink) -> Result<RunOutcome> {
        self.run_with(spec, sink, RunInputs::default())
    }

    pub(crate) fn run_with(
        &mut self,
        spec: RunSpec,
        sink: &mut dyn OutputSink,
        inputs: RunInputs,
    ) -> Result<RunOutcome> {
        let stream = self.stream_mut();
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        let request = Frame::Request(RequestFrame::Run {
            spec: to_wire(spec)?,
        });
        write_frame(stream, &request)?;
        let writer = Arc::new(Mutex::new(stream.try_clone()?));
        // Workers hold clones of the socket. Shutting it down on every return lets
        // the daemon see the client leave and stop the child.
        let _shutdown = ShutdownOnDrop(stream.try_clone()?);
        let credit = Arc::new(Credit::default());
        let _close_credit = CloseOnDrop(Arc::clone(&credit));
        let completion = Arc::new(Completion::default());
        let stdin = StdinWorker {
            writer: Arc::clone(&writer),
            credit: Arc::clone(&credit),
            completion: Arc::clone(&completion),
            abort: stream.try_clone()?,
        };
        stdin.spawn(inputs.stdin);
        if let Some(signals) = inputs.signals {
            forward_signals(writer, signals);
        }
        completion.settle(relay(stream, sink, &credit))
    }
}

/// Deliver output until the daemon reports the exit or the connection fails.
fn relay(
    stream: &mut UnixStream,
    sink: &mut dyn OutputSink,
    credit: &Credit,
) -> Result<RunOutcome> {
    loop {
        match read_frame(stream)? {
            Frame::Response(ResponseFrame::RunOutput { stream: kind, data }) => match kind {
                StreamKind::Stdout => sink.stdout(&data.0)?,
                StreamKind::Stderr => sink.stderr(&data.0)?,
            },
            Frame::Response(ResponseFrame::RunNotice { message }) => sink.notice(&message)?,
            Frame::Response(ResponseFrame::RunStdinCredit { bytes }) => {
                credit.grant(bytes as usize)
            }
            Frame::Response(ResponseFrame::RunExit { outcome }) => return Ok(outcome),
            Frame::Response(ResponseFrame::Error { message, .. }) => bail!("{message}"),
            _ => bail!("unexpected frame from daemon"),
        }
    }
}

pub(super) fn to_wire(spec: RunSpec) -> Result<WireRunSpec> {
    let bindings = spec
        .bindings
        .into_iter()
        .map(|binding| match binding.secret {
            SecretRef::Key { key, project } => Ok(WireBinding {
                key,
                project,
                env: binding.env,
                delivery: binding.delivery,
            }),
            SecretRef::Id(_) => bail!("run bindings name a secret by key"),
        })
        .collect::<Result<_>>()?;
    let bytes = |value: &std::ffi::OsStr| WireBytes(value.as_bytes().to_vec());
    Ok(WireRunSpec {
        bindings,
        program: bytes(&spec.program),
        args: spec.args.iter().map(|arg| bytes(arg)).collect(),
        cwd: bytes(spec.cwd.as_os_str()),
        env: spec
            .env_passthrough
            .into_iter()
            .map(|(name, value)| (name, bytes(&value)))
            .collect(),
    })
}

fn send(writer: &Mutex<UnixStream>, frame: RequestFrame) -> Result<()> {
    let mut stream = writer
        .lock()
        .map_err(|_| anyhow!("the run connection lock is poisoned"))?;
    write_frame(&mut *stream, &Frame::Request(frame))
}

/// Why the stdin worker stopped early.
enum InputFailure {
    /// The child may have taken the partial input as complete, so this
    /// outranks even a reported exit.
    Read(std::io::Error),
    /// The connection broke. A reported exit still stands.
    Send(anyhow::Error),
}

/// Settles a run once: the daemon's reply against a stdin worker failure.
#[derive(Default)]
struct Completion(Mutex<Settled>);

#[derive(Default)]
enum Settled {
    #[default]
    Running,
    Failed(InputFailure),
    Done,
}

impl Completion {
    fn fail(&self, failure: InputFailure) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*state, Settled::Running) {
            *state = Settled::Failed(failure);
        }
    }

    fn settle(&self, result: Result<RunOutcome>) -> Result<RunOutcome> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match std::mem::replace(&mut *state, Settled::Done) {
            Settled::Failed(InputFailure::Read(error)) => {
                Err(anyhow::Error::new(error).context("reading stdin for the run failed"))
            }
            Settled::Failed(InputFailure::Send(error)) if result.is_err() => {
                Err(error.context("sending stdin to the daemon failed"))
            }
            _ => result,
        }
    }
}

/// Stdin bytes the daemon has agreed to take. The stdin thread waits for it.
#[derive(Default)]
struct Credit {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}

impl Credit {
    fn grant(&self, bytes: usize) {
        if let Ok(mut state) = self.state.lock() {
            state.0 += bytes;
            self.changed.notify_all();
        }
    }

    /// Wait for credit. `None` once the run is over.
    fn wait(&self) -> Option<usize> {
        let mut state = self.state.lock().ok()?;
        while state.0 == 0 && !state.1 {
            state = self.changed.wait(state).ok()?;
        }
        (!state.1).then_some(state.0)
    }

    fn spend(&self, bytes: usize) {
        if let Ok(mut state) = self.state.lock() {
            state.0 -= bytes;
        }
    }
}

struct CloseOnDrop(Arc<Credit>);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.1 = true;
            self.0.changed.notify_all();
        }
    }
}

/// A non-blocking stdin with no data yet is polled at this pace. The credit
/// check before each read ends the worker once the run is over.
const WOULD_BLOCK_RETRY: Duration = Duration::from_millis(20);

struct StdinWorker {
    writer: Arc<Mutex<UnixStream>>,
    credit: Arc<Credit>,
    completion: Arc<Completion>,
    /// Closes the connection after a failure without the writer lock, which a
    /// blocked send may hold.
    abort: UnixStream,
}

impl StdinWorker {
    fn spawn(self, stdin: Option<Box<dyn Read + Send>>) {
        std::thread::spawn(move || {
            if let Err(failure) = self.forward(stdin) {
                self.completion.fail(failure);
                let _ = self.abort.shutdown(Shutdown::Both);
            }
        });
    }

    /// Send stdin within the credit window, then end of input.
    fn forward(&self, stdin: Option<Box<dyn Read + Send>>) -> Result<(), InputFailure> {
        if let Some(mut stdin) = stdin {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                let Some(available) = self.credit.wait() else {
                    return Ok(());
                };
                let limit = available.min(buf.len());
                match stdin.read(&mut buf[..limit]) {
                    Ok(0) => break,
                    Ok(read) => {
                        self.credit.spend(read);
                        let data = WireBytes(buf[..read].to_vec());
                        send(&self.writer, RequestFrame::RunStdin { data })
                            .map_err(InputFailure::Send)?;
                    }
                    Err(error) if error.kind() == ErrorKind::Interrupted => {}
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(WOULD_BLOCK_RETRY)
                    }
                    Err(error) => return Err(InputFailure::Read(error)),
                }
            }
        }
        send(&self.writer, RequestFrame::RunStdinEof).map_err(InputFailure::Send)
    }
}

struct ShutdownOnDrop(UnixStream);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

fn forward_signals(writer: Arc<Mutex<UnixStream>>, signals: Receiver<ForwardSignal>) {
    std::thread::spawn(move || {
        for signal in signals {
            if send(&writer, RequestFrame::RunSignal { signal }).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests;
