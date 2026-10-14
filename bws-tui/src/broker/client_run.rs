//! `Client::run`. The client sends names and its own context, then relays
//! masked output. It never receives a secret value.

use super::client::Client;
use super::meta::SecretRef;
use super::protocol::{
    read_frame, write_frame, Frame, RequestFrame, ResponseFrame, WireBinding, WireBytes,
    WireRunSpec,
};
use super::runspec::{ForwardSignal, OutputSink, RunOutcome, RunSpec, StreamKind};
use anyhow::{bail, Result};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};

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
        let credit = Arc::new(Credit::default());
        let _close_credit = CloseOnDrop(Arc::clone(&credit));
        forward_stdin(Arc::clone(&writer), inputs.stdin, Arc::clone(&credit));
        if let Some(signals) = inputs.signals {
            forward_signals(writer, signals);
        }
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
}

fn to_wire(spec: RunSpec) -> Result<WireRunSpec> {
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

fn send(writer: &Mutex<UnixStream>, frame: RequestFrame) -> bool {
    match writer.lock() {
        Ok(mut stream) => write_frame(&mut *stream, &Frame::Request(frame)).is_ok(),
        Err(_) => false,
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

fn forward_stdin(
    writer: Arc<Mutex<UnixStream>>,
    stdin: Option<Box<dyn Read + Send>>,
    credit: Arc<Credit>,
) {
    std::thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            let mut buf = vec![0u8; 32 * 1024];
            while let Some(available) = credit.wait() {
                let limit = available.min(buf.len());
                match stdin.read(&mut buf[..limit]) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        credit.spend(read);
                        let data = WireBytes(buf[..read].to_vec());
                        if !send(&writer, RequestFrame::RunStdin { data }) {
                            return;
                        }
                    }
                }
            }
        }
        send(&writer, RequestFrame::RunStdinEof);
    });
}

fn forward_signals(writer: Arc<Mutex<UnixStream>>, signals: Receiver<ForwardSignal>) {
    std::thread::spawn(move || {
        for signal in signals {
            if !send(&writer, RequestFrame::RunSignal { signal }) {
                return;
            }
        }
    });
}
