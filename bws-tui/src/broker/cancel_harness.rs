//! Drives the production human handler over a real socket pair, so a test
//! decides exactly when the client hangs up. Frames, passed descriptors, and
//! the handler are production code. Only the vault and audit are stand-ins.

use super::audit::{AuditLog, HumanAudit, RunAudit};
use super::backend::VaultBackend;
use super::human::{serve_human, HumanConfig, HumanRequest};
use super::operation::Operation;
use super::peercred::{read_peer_credentials, PeerCredentials};
use super::protocol::{read_frame_with_rights, write_frame_with_rights};
use super::protocol::{Frame, RequestFrame, ResponseFrame};
use std::os::fd::BorrowedFd;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

/// Records nothing. These tests are about cancellation, not the audit line.
struct NoAudit;

impl AuditLog for NoAudit {
    fn record_catalog(&self, _: PeerCredentials, _: Option<&str>) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_run(&self, _: PeerCredentials, _: &RunAudit<'_>) -> anyhow::Result<()> {
        Ok(())
    }

    fn record_human(&self, _: PeerCredentials, _: &HumanAudit<'_>) -> anyhow::Result<()> {
        Ok(())
    }
}

/// One human request already on the wire. The client end stays here.
pub(super) struct Pending {
    client: Option<UnixStream>,
    daemon: Option<(UnixStream, PeerCredentials)>,
    reply: Option<Receiver<ResponseFrame>>,
}

impl Pending {
    /// Send `op` with `fds`, as the client does. Nothing is served yet.
    pub(super) fn new(op: Operation, fds: &[BorrowedFd<'_>]) -> Self {
        let (client, daemon) = UnixStream::pair().unwrap();
        let frame = Frame::Request(RequestFrame::Human { op });
        write_frame_with_rights(&client, &frame, fds).unwrap();
        // Read at accept time, as the daemon does. macOS refuses once the peer left.
        let peer = read_peer_credentials(&daemon).unwrap();
        Self {
            client: Some(client),
            daemon: Some((daemon, peer)),
            reply: None,
        }
    }

    /// Serve the request on its own worker thread.
    pub(super) fn serve(
        mut self,
        backend: Arc<dyn VaultBackend>,
        config: Arc<HumanConfig>,
    ) -> Self {
        let (daemon, peer) = self.daemon.take().expect("served once");
        let (sender, reply) = channel();
        std::thread::spawn(move || {
            let received = read_frame_with_rights(&daemon).unwrap();
            let Frame::Request(RequestFrame::Human { op }) = received.frame else {
                panic!("not a human frame");
            };
            let request = HumanRequest {
                op,
                fds: received.fds,
                violation: received.violation,
                peer,
                socket: &daemon,
            };
            let _ = sender.send(serve_human(request, &*backend, &NoAudit, &config));
        });
        self.reply = Some(reply);
        self
    }

    /// The client hangs up, as when the user presses Ctrl-C in `hush add`.
    pub(super) fn hang_up(&mut self) {
        self.client = None;
    }

    /// The handler's reply, or `None` when it did not return within `bound`.
    pub(super) fn reply_within(&self, bound: Duration) -> Option<ResponseFrame> {
        let reply = self.reply.as_ref().expect("serve first");
        reply.recv_timeout(bound).ok()
    }
}
