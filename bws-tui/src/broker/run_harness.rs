//! Shared harness for `hush run` tests: a same-user daemon on a test socket,
//! a fake vault, and `/bin/sh` children.

use super::meta::{SecretMeta, SecretRef};
use super::run_prepare::RunConfig;
use super::runspec::{Binding, Delivery, EnvName, OutputSink, RunSpec};
use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(super) const ENV_VALUE: &str = "FAKE-run-value-0001";
pub(super) const FD_VALUE: &str = "FAKE-fd-value-0002";

pub(super) fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-run-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn backend() -> FakeBackend {
    backend_with(&[])
}

/// The standard fake vault plus extra `(id, key, value)` entries.
pub(super) fn backend_with(extra: &[(&str, &str, String)]) -> FakeBackend {
    let entry = |id: &str, key: &str, value: &str| {
        let meta = SecretMeta {
            id: id.into(),
            key: key.into(),
            project: Some("Test".into()),
            project_id: Some("p1".into()),
            note: String::new(),
        };
        (id.to_string(), (meta, value.to_string()))
    };
    let mut secrets = HashMap::from([
        entry("s1", "TOKEN", ENV_VALUE),
        entry("s2", "DB_PASSWORD", FD_VALUE),
        entry("s3", "TINY", "tiny1"),
    ]);
    secrets.extend(extra.iter().map(|(id, key, value)| entry(id, key, value)));
    FakeBackend::new(secrets)
}

pub(super) fn daemon(dir: &Path, run: RunConfig) -> Client {
    Client::connect_to(&daemon_socket(dir, run)).unwrap()
}

/// A daemon that serves exactly one connection.
pub(super) fn daemon_socket(dir: &Path, run: RunConfig) -> PathBuf {
    serve_one(dir, run, backend()).0
}

/// Serve one connection from `vault`. The receiver fires once its handler returns.
pub(super) fn serve_one(
    dir: &Path,
    run: RunConfig,
    vault: FakeBackend,
) -> (PathBuf, std::sync::mpsc::Receiver<()>) {
    let socket = dir.join("hush.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let audit = audit::FileAuditLog::open(&dir.join("audit.log")).unwrap();
    let (done_tx, done) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let human = human::HumanConfig::default();
        let _ = daemon::serve_connection_with(stream, &vault, &audit, &run, &human);
        let _ = done_tx.send(());
    });
    (socket, done)
}

pub(super) fn binding(key: &str, env: &str, delivery: Delivery) -> Binding {
    Binding {
        secret: SecretRef::Key {
            key: key.into(),
            project: None,
        },
        env: EnvName::try_from(env.to_string()).unwrap(),
        delivery,
    }
}

pub(super) fn sh(bindings: Vec<Binding>, script: &str, cwd: &Path) -> RunSpec {
    RunSpec {
        bindings,
        program: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        cwd: cwd.to_path_buf(),
        env_passthrough: vec![(
            EnvName::try_from("PATH".to_string()).unwrap(),
            "/usr/bin:/bin".into(),
        )],
    }
}

#[derive(Default)]
pub(super) struct Capture {
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    pub notices: Vec<String>,
}

impl Capture {
    pub(super) fn text(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }
}

impl OutputSink for Capture {
    fn stdout(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.out.extend_from_slice(bytes);
        Ok(())
    }
    fn stderr(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.err.extend_from_slice(bytes);
        Ok(())
    }
    fn notice(&mut self, message: &str) -> std::io::Result<()> {
        self.notices.push(message.to_owned());
        Ok(())
    }
}

pub(super) fn wait_until(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
