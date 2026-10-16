//! A test daemon with a fake password check. The fake stands in for PAM and
//! OpenDirectory only. Sockets, descriptors, and terminals stay real.

use super::approve::{Authenticator, Verdict};
use super::human::HumanConfig;
use super::{audit, daemon, FakeBackend};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub(super) const PASSWORD: &str = "correct horse battery";

/// Accepts [`PASSWORD`] and counts every check. A panicking one proves a path never asks.
pub(super) struct FakeAuth {
    calls: AtomicUsize,
    panics: bool,
}

impl FakeAuth {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            panics: false,
        })
    }

    pub(super) fn panicking() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            panics: true,
        })
    }

    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Authenticator for FakeAuth {
    fn available(&self) -> Result<(), String> {
        assert!(!self.panics, "this operation must not ask for approval");
        Ok(())
    }

    fn verify(&self, _account: &str, password: &[u8]) -> Result<Verdict, String> {
        assert!(!self.panics, "this operation must not ask for approval");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(if password == PASSWORD.as_bytes() {
            Verdict::Approved
        } else {
            Verdict::Denied
        })
    }
}

/// A daemon that serves `connections` connections, each on its own thread.
pub(super) fn human_daemon(
    dir: &Path,
    backend: Arc<FakeBackend>,
    auth: Arc<dyn Authenticator>,
    connections: usize,
) -> (PathBuf, std::thread::JoinHandle<()>) {
    let socket = dir.join("hush.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let audit = Arc::new(audit::FileAuditLog::open(&dir.join("audit.log")).unwrap());
    let human = Arc::new(HumanConfig::new(auth));
    let handle = std::thread::spawn(move || {
        let workers: Vec<_> = (0..connections)
            .map(|_| {
                let (stream, _) = listener.accept().unwrap();
                let (backend, audit, human) = (backend.clone(), audit.clone(), human.clone());
                std::thread::spawn(move || {
                    let run = super::run_prepare::RunConfig::default();
                    let _ = daemon::serve_connection_with(stream, &*backend, &*audit, &run, &human);
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    });
    (socket, handle)
}
