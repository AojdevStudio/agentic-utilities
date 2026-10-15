//! Broker tests. Each test names one PR 1 behavior.
use super::*;
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const FAKE_VALUE: &str = "FAKE-SECRET-VALUE-must-never-cross-wire";

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hush-broker-test-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn protocol_frame_round_trip() {
    let frame = Frame::Request(RequestFrame::Catalog {
        project: Some("API Keys".into()),
    });
    let mut buf = Vec::new();
    protocol::write_frame(&mut buf, &frame).unwrap();
    let back = protocol::read_frame(&mut Cursor::new(buf)).unwrap();
    assert_eq!(back, frame);
}

#[test]
fn protocol_rejects_wrong_version_and_oversized_frame() {
    let frame = Frame::Request(RequestFrame::Catalog { project: None });
    let mut buf = Vec::new();
    protocol::write_frame(&mut buf, &frame).unwrap();
    let mut wrong_version = buf.clone();
    wrong_version[0] = 0xff;
    assert!(protocol::read_frame(&mut Cursor::new(wrong_version)).is_err());
    let mut oversized = Vec::new();
    oversized.push(protocol::PROTOCOL_VERSION);
    oversized.extend_from_slice(&u32::MAX.to_be_bytes()[..3]);
    assert!(protocol::read_frame(&mut Cursor::new(oversized)).is_err());
}

#[test]
fn no_frame_can_carry_secret_material() {
    static_assertions::assert_not_impl_any!(material::SecretMaterial: serde::Serialize, Clone, std::fmt::Debug, std::fmt::Display);
    let frames = [
        Frame::Request(RequestFrame::Catalog {
            project: Some("API Keys".into()),
        }),
        Frame::Response(ResponseFrame::Catalog {
            entries: vec![meta::SecretMeta {
                id: "s1".into(),
                key: "TOKEN".into(),
                project: Some("API Keys".into()),
                project_id: Some("p1".into()),
                note: "scope: test".into(),
            }],
        }),
        Frame::Response(ResponseFrame::Error {
            code: "not_found".into(),
            message: "no such secret".into(),
        }),
    ];
    for frame in frames {
        let mut buf = Vec::new();
        protocol::write_frame(&mut buf, &frame).unwrap();
        let text = String::from_utf8_lossy(&buf).into_owned();
        assert!(!text.contains(FAKE_VALUE), "frame leaked secret value");
        assert!(!text.contains("\"value\""), "frame has a value field");
    }
}

#[test]
fn token_file_check_refuses_wrong_mode() {
    let dir = tempdir("token-mode");
    let path = dir.join("token");
    fs::write(&path, "tok\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(token::load_checked(&path, true).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(token::load_checked(&path, true).is_ok());
}

#[test]
fn token_file_check_refuses_empty_and_wrong_owner() {
    let dir = tempdir("token-empty");
    let path = dir.join("token");
    fs::write(&path, "\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(token::load_checked(&path, true).is_err());
    let euid = unsafe { libc::geteuid() };
    assert!(hardening::check_token_metadata(0o600, 999_999, euid, true).is_err());
    assert!(hardening::check_token_metadata(0o600, euid, euid, true).is_ok());
}

fn fake_backend() -> FakeBackend {
    let mut secrets = HashMap::new();
    secrets.insert(
        "s1".to_string(),
        (
            meta::SecretMeta {
                id: "s1".into(),
                key: "TOKEN".into(),
                project: Some("API Keys".into()),
                project_id: Some("p1".into()),
                note: "scope: test".into(),
            },
            FAKE_VALUE.to_string(),
        ),
    );
    secrets.insert(
        "s2".to_string(),
        (
            meta::SecretMeta {
                id: "s2".into(),
                key: "DB_PASSWORD".into(),
                project: Some("Databases".into()),
                project_id: Some("p2".into()),
                note: String::new(),
            },
            "another-fake-value".to_string(),
        ),
    );
    FakeBackend::new(secrets)
}

#[test]
fn daemon_client_catalog_returns_metadata_without_values_on_wire() {
    let dir = tempdir("daemon-catalog");
    let socket = dir.join("hush.sock");
    let audit_path = dir.join("audit.log");
    let audit = Arc::new(audit::FileAuditLog::open(&audit_path).unwrap());
    let backend = Arc::new(fake_backend());
    let server_socket = socket.clone();
    let server_audit = Arc::clone(&audit);
    let server_backend = Arc::clone(&backend);
    let listener = std::os::unix::net::UnixListener::bind(&server_socket).unwrap();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        daemon::serve_connection(stream, &*server_backend, &*server_audit).unwrap();
    });
    let captured = tee_connect(&socket);
    let mut client = client::Client::from_stream(captured.stream).unwrap();
    let metas = client.catalog(Some("API Keys")).unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].key, "TOKEN");
    drop(client);
    let wire = Tee::take_bytes(captured.bytes, captured.pump);
    let wire_text = String::from_utf8_lossy(&wire);
    assert!(
        !wire_text.contains(FAKE_VALUE),
        "wire carried a secret value: {wire_text}"
    );
    assert!(!wire_text.contains("another-fake-value"));
    handle.join().unwrap();
    let log = fs::read_to_string(&audit_path).unwrap();
    assert!(
        log.contains("catalog"),
        "audit log missing catalog record: {log}"
    );
    assert!(log.contains("uid="), "audit log missing peer uid: {log}");
    assert!(log.contains("pid="), "audit log missing peer pid: {log}");
    assert!(!log.contains(FAKE_VALUE));
}

struct Tee {
    stream: UnixStream,
    bytes: Arc<Mutex<Vec<u8>>>,
    pump: Option<std::thread::JoinHandle<()>>,
}

impl Tee {
    fn take_bytes(
        bytes: Arc<Mutex<Vec<u8>>>,
        pump: Option<std::thread::JoinHandle<()>>,
    ) -> Vec<u8> {
        if let Some(pump) = pump {
            let _ = pump.join();
        }
        bytes.lock().unwrap().clone()
    }
}

fn tee_connect(socket: &Path) -> Tee {
    // A proxy thread owns the real socket. It copies bytes both ways and
    // records everything the daemon sent, which is the wire evidence.
    use std::os::unix::net::UnixListener;
    let proxy_dir = socket.parent().unwrap().join("proxy");
    fs::create_dir_all(&proxy_dir).unwrap();
    let proxy_socket = proxy_dir.join("s.sock");
    let _ = fs::remove_file(&proxy_socket);
    let listener = UnixListener::bind(&proxy_socket).unwrap();
    let daemon_socket = socket.to_path_buf();
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&bytes);
    let pump = std::thread::spawn(move || {
        let (mut client_side, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(_) => return,
        };
        let mut daemon_side = match UnixStream::connect(&daemon_socket) {
            Ok(stream) => stream,
            Err(_) => return,
        };
        let mut to_daemon = match daemon_side.try_clone() {
            Ok(stream) => stream,
            Err(_) => return,
        };
        let mut from_client = match client_side.try_clone() {
            Ok(stream) => stream,
            Err(_) => return,
        };
        let up = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match from_client.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = to_daemon.shutdown(std::net::Shutdown::Both);
                        break;
                    }
                    Ok(n) => {
                        if to_daemon.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut buf = [0u8; 4096];
        loop {
            match daemon_side.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = client_side.shutdown(std::net::Shutdown::Both);
                    break;
                }
                Ok(n) => {
                    sink.lock().unwrap().extend_from_slice(&buf[..n]);
                    if client_side.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = up.join();
    });
    let stream = UnixStream::connect(&proxy_socket).unwrap();
    Tee {
        stream,
        bytes,
        pump: Some(pump),
    }
}

#[test]
fn peer_credentials_are_read_from_kernel() {
    let dir = tempdir("peercred");
    let socket = dir.join("hush.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let mut client_stream = UnixStream::connect(&socket).unwrap();
    let (mut accepted, _) = listener.accept().unwrap();
    client_stream.write_all(b"x").unwrap();
    let mut one = [0u8; 1];
    accepted.read_exact(&mut one).unwrap();
    let creds = peercred::read_peer_credentials(&accepted).unwrap();
    assert_eq!(creds.uid as u32, unsafe { libc::geteuid() });
    assert_eq!(creds.gid as u32, unsafe { libc::getegid() });
    assert_eq!(creds.pid as i32, unsafe { libc::getpid() });
}

#[test]
fn fake_backend_fetch_returns_value_only_via_secret_material() {
    let backend = fake_backend();
    let control = super::request::RequestControl::detached();
    let material = backend
        .fetch(&meta::SecretRef::Id("s1".into()), &control)
        .unwrap();
    assert_eq!(material.expose_bytes(), FAKE_VALUE.as_bytes());
    assert!(backend
        .fetch(&meta::SecretRef::Id("missing".into()), &control)
        .is_err());
}
