//! Socket server. Holds the token, the catalog, and every secret value.
//!
//! In PR 1 the daemon has no filesystem identity check on the peer beyond
//! recording kernel credentials. Policy gates arrive in later PRs.

use super::audit::{AuditLog, FileAuditLog};
use super::backend::{BwsBackend, VaultBackend};
use super::paths::{default_token_path, resolve_on_path};
use super::peercred::read_peer_credentials;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use super::token::load_checked;
use anyhow::{Context, Result};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;

/// Options for the hidden `daemon` subcommand.
pub(crate) struct DaemonOptions {
    pub socket: PathBuf,
    pub token_file: Option<PathBuf>,
    pub audit_log: Option<PathBuf>,
    pub bws_path: Option<PathBuf>,
}

/// Serve one connection until the client disconnects. Exposed for tests.
pub(crate) fn serve_connection(
    mut stream: UnixStream,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
) -> Result<()> {
    let peer = read_peer_credentials(&stream)?;
    loop {
        let frame = match read_frame(&mut stream) {
            Ok(frame) => frame,
            Err(error) if is_clean_eof(&error) => return Ok(()),
            Err(error) => return Err(error),
        };
        let response = match frame {
            Frame::Request(RequestFrame::Catalog { project }) => {
                audit.record_catalog(peer, project.as_deref())?;
                match backend.catalog() {
                    Ok(entries) => ResponseFrame::Catalog {
                        entries: entries
                            .into_iter()
                            .filter(|meta| project.as_deref().is_none_or(|q| meta.in_project(q)))
                            .collect(),
                    },
                    Err(error) => ResponseFrame::Error {
                        code: "catalog_failed".into(),
                        message: error.to_string(),
                    },
                }
            }
            Frame::Response(_) => ResponseFrame::Error {
                code: "unexpected_frame".into(),
                message: "clients send requests".into(),
            },
        };
        write_frame(&mut stream, &Frame::Response(response))?;
    }
}

fn is_clean_eof(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| io.kind() == std::io::ErrorKind::UnexpectedEof)
}

pub(crate) fn run_daemon(options: DaemonOptions) -> Result<()> {
    let token_path = options.token_file.unwrap_or_else(default_token_path);
    let token = load_checked(&token_path)?;
    let bws_path = match options.bws_path {
        Some(path) => path,
        None => resolve_on_path("bws")?,
    };
    let backend: Arc<dyn VaultBackend> = Arc::new(BwsBackend::new(bws_path, token));
    backend.catalog().context("initial catalog build failed")?;
    let audit_path = options
        .audit_log
        .unwrap_or_else(super::audit::default_audit_path);
    let audit = Arc::new(FileAuditLog::open(&audit_path)?);
    run_server(options.socket, backend.as_ref(), audit.as_ref())
}

fn run_server(socket: PathBuf, backend: &dyn VaultBackend, audit: &dyn AuditLog) -> Result<()> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("failed to bind daemon socket at {}", socket.display()))?;
    eprintln!("hush daemon listening on {}", socket.display());
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = serve_connection(stream, backend, audit) {
                    eprintln!("connection failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}
