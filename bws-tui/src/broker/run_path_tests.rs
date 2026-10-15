//! PATH search preparation stays inside one request's worth of memory. A
//! search that would need more is refused before any value is fetched, and
//! the daemon keeps serving.

use super::audit::FileAuditLog;
use super::backend::{FakeBackend, VaultBackend};
use super::client_run::to_wire;
use super::material::SecretMaterial;
use super::meta::{SecretMeta, SecretRef};
use super::peercred::PeerCredentials;
use super::run_harness::*;
use super::run_prepare::{prepare_run, RunConfig};
use super::runspec::{Delivery, EnvName, RunOutcome, RunSpec};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting {
    inner: FakeBackend,
    fetches: AtomicUsize,
}

impl VaultBackend for Counting {
    fn catalog(&self) -> anyhow::Result<Vec<SecretMeta>> {
        self.inner.catalog()
    }
    fn fetch(&self, secret: &SecretRef) -> anyhow::Result<SecretMaterial> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        self.inner.fetch(secret)
    }
}

fn searching(program: &str, path: String, cwd: &Path) -> RunSpec {
    let mut spec = sh(vec![binding("TOKEN", "TOKEN", Delivery::Env)], "", cwd);
    spec.program = program.into();
    spec.args.clear();
    spec.env_passthrough = vec![(EnvName::try_from("PATH".to_string()).unwrap(), path.into())];
    spec
}

fn assert_refused(error: &anyhow::Error) {
    let text = format!("{error:#}");
    assert!(
        text.contains("program search") && !text.contains("failed to start"),
        "expected the preparation refusal, got: {text}"
    );
}

#[test]
fn an_oversized_path_search_is_refused_and_the_daemon_keeps_serving() {
    let dir = tempdir("path-budget");
    let mut deep = dir.clone();
    for level in 0..5 {
        deep.push(format!("{level}{}", "d".repeat(200)));
    }
    std::fs::create_dir_all(&deep).unwrap();
    let mut client = daemon(&dir, RunConfig::default());

    let empty_entries = ":".repeat(2047);
    let error = client
        .run(
            searching("tool", empty_entries, &deep),
            &mut Capture::default(),
        )
        .unwrap_err();
    assert_refused(&error);

    let long_name = "a".repeat(64 * 1024);
    let entries = vec!["/usr/bin"; 32].join(":");
    let error = client
        .run(
            searching(&long_name, entries, &dir),
            &mut Capture::default(),
        )
        .unwrap_err();
    assert_refused(&error);

    let mut sink = Capture::default();
    let spec = sh(
        vec![binding("TOKEN", "TOKEN", Delivery::Env)],
        "echo ok",
        &dir,
    );
    assert_eq!(client.run(spec, &mut sink).unwrap(), RunOutcome::Exited(0));
    assert_eq!(sink.text(), "ok\n");
}

#[test]
fn many_short_path_entries_count_their_overhead_and_fetch_nothing() {
    let dir = tempdir("path-overhead");
    let vault = Counting {
        inner: backend(),
        fetches: AtomicUsize::new(0),
    };
    let audit = FileAuditLog::open(&dir.join("audit.log")).unwrap();
    // SAFETY: these id getters take no arguments and cannot fail.
    let peer = unsafe {
        PeerCredentials {
            uid: libc::getuid(),
            gid: libc::getgid(),
            pid: libc::getpid(),
        }
    };
    let spec = to_wire(searching("t", vec!["/"; 30_000].join(":"), &dir)).unwrap();
    let error = prepare_run(peer, spec, &vault, &audit, &RunConfig::default())
        .err()
        .expect("30000 PATH entries must not prepare");
    assert_refused(&error);
    assert_eq!(
        vault.fetches.load(Ordering::SeqCst),
        0,
        "fetched before refusing"
    );
}
