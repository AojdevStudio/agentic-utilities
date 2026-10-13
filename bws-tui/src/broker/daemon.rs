//! Socket server. Holds the token, the catalog, and every secret value.

use super::audit::{AuditLog, FileAuditLog};
use super::backend::{BwsBackend, VaultBackend};
use super::paths::{default_token_path, resolve_trusted_bws};
use super::peercred::read_peer_credentials;
use super::protocol::{read_frame, write_frame, Frame, RequestFrame, ResponseFrame};
use super::token::load_checked;
use anyhow::{bail, Context, Result};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Options for the hidden `daemon` subcommand.
pub(crate) struct DaemonOptions {
    pub socket: PathBuf,
    pub token_file: Option<PathBuf>,
    pub audit_log: Option<PathBuf>,
    pub bws_path: Option<PathBuf>,
    pub dev_same_user: bool,
}

/// Per connection limits shared by every worker thread.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ServeConfig {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub max_connections: usize,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            max_connections: 32,
        }
    }
}

/// Serve one connection after setting stream timeouts.
pub(crate) fn serve_connection_guarded(
    stream: UnixStream,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    config: ServeConfig,
) -> Result<()> {
    stream.set_read_timeout(Some(config.read_timeout))?;
    stream.set_write_timeout(Some(config.write_timeout))?;
    serve_connection(stream, backend, audit)
}

/// Serve one connection until the client disconnects.
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

/// Clean disconnect check for a frame read failure.
fn is_clean_eof(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| io.kind() == std::io::ErrorKind::UnexpectedEof)
}

/// Pure daemon start rule. A non-root euid needs the dev allowance.
pub(crate) fn check_daemon_euid(euid: u32, dev_same_user: bool) -> Result<()> {
    if !dev_same_user && euid != 0 {
        #[cfg(debug_assertions)]
        {
            bail!("hush daemon must run as root. Pass --dev-same-user for same-user development");
        }
        #[cfg(not(debug_assertions))]
        {
            bail!("hush daemon must run as root");
        }
    }
    Ok(())
}

/// Start the daemon. Without `dev_same_user` the daemon must run as root.
pub(crate) fn run_daemon(options: DaemonOptions) -> Result<()> {
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    check_daemon_euid(euid, options.dev_same_user)?;
    let token_path = options.token_file.unwrap_or_else(default_token_path);
    let token = load_checked(&token_path, options.dev_same_user)?;
    let bws_path = resolve_trusted_bws(options.bws_path, options.dev_same_user)?;
    let backend: Arc<dyn VaultBackend> = Arc::new(BwsBackend::new(bws_path, token));
    backend.catalog().context("initial catalog build failed")?;
    let audit_path = options
        .audit_log
        .unwrap_or_else(super::audit::default_audit_path);
    let audit_raw = FileAuditLog::open(&audit_path)?;
    let audit: Arc<dyn AuditLog> = Arc::new(audit_raw);
    let listener = super::server::bind_listener(&options.socket)?;
    eprintln!("hush daemon listening on {}", options.socket.display());
    super::server::run_server(listener, backend, audit, ServeConfig::default())
}
