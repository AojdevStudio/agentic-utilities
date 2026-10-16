//! Socket server. Holds the token, the catalog, and every secret value.

use super::audit::{AuditLog, FileAuditLog};
use super::backend::{BwsBackend, VaultBackend};
use super::human::{serve_human, HumanConfig, HumanRequest};
use super::operation::{DescriptorProblem, Refusal};
use super::paths::{default_token_path, resolve_trusted_bws};
use super::peercred::read_peer_credentials;
use super::policy::{default_policy_path, Policy};
use super::protocol::{
    read_frame_with_rights, write_frame, Frame, Received, RequestFrame, ResponseFrame,
};
use super::request::RequestControl;
use super::run_prepare::{prepare_run, RunConfig};
use super::spawn_fd::FdCeiling;
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
    pub policy: Option<PathBuf>,
    pub dev_same_user: bool,
}

/// Most connections served at once. Child tables are sized from it too.
pub(crate) const MAX_CONNECTIONS: usize = 32;

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
            max_connections: MAX_CONNECTIONS,
        }
    }
}

/// Serve one connection after setting stream timeouts.
pub(crate) fn serve_connection_guarded(
    stream: UnixStream,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    config: ServeConfig,
    run: &RunConfig,
    human: &HumanConfig,
) -> Result<()> {
    stream.set_read_timeout(Some(config.read_timeout))?;
    stream.set_write_timeout(Some(config.write_timeout))?;
    serve_connection_with(stream, backend, audit, run, human)
}

/// Serve one connection with the default run and human settings.
#[cfg(test)]
pub(crate) fn serve_connection(
    stream: UnixStream,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
) -> Result<()> {
    serve_connection_with(
        stream,
        backend,
        audit,
        &RunConfig::default(),
        &HumanConfig::default(),
    )
}

/// Serve one connection until the client disconnects. A run request takes
/// over the connection until the child exits. Descriptors are accepted only
/// on human operations, and only in the roles the operation names.
pub(crate) fn serve_connection_with(
    mut stream: UnixStream,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    run: &RunConfig,
    human: &HumanConfig,
) -> Result<()> {
    let peer = read_peer_credentials(&stream)?;
    loop {
        let Received {
            frame,
            fds,
            violation,
        } = match read_frame_with_rights(&stream) {
            Ok(received) => received,
            Err(error) if is_clean_eof(&error) => return Ok(()),
            Err(error) => return Err(error),
        };
        let response = match frame {
            Frame::Request(RequestFrame::Human { op }) => {
                let request = HumanRequest {
                    op,
                    fds,
                    violation,
                    peer,
                    socket: &stream,
                };
                serve_human(request, backend, audit, human)
            }
            _ if violation.is_some() || !fds.is_empty() => ResponseFrame::Refused {
                refusal: Refusal::Descriptors {
                    problem: DescriptorProblem::Malformed,
                },
            },
            Frame::Request(RequestFrame::Catalog { project }) => {
                audit.record_catalog(peer, project.as_deref())?;
                match backend.catalog(&RequestControl::detached()) {
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
            Frame::Request(RequestFrame::Projects) => {
                match backend.projects(&RequestControl::detached()) {
                    Ok(projects) => ResponseFrame::Projects { projects },
                    Err(error) => ResponseFrame::Error {
                        code: "projects_failed".into(),
                        message: error.to_string(),
                    },
                }
            }
            Frame::Request(RequestFrame::Run { spec }) => {
                match prepare_run(peer, spec, backend, audit, run) {
                    Ok(prepared) => return super::run_loop::serve_run(stream, prepared),
                    Err(error) => ResponseFrame::Error {
                        code: "run_refused".into(),
                        message: error.to_string(),
                    },
                }
            }
            Frame::Request(
                RequestFrame::RunStdin { .. }
                | RequestFrame::RunStdinEof
                | RequestFrame::RunSignal { .. },
            ) => ResponseFrame::Error {
                code: "unexpected_frame".into(),
                message: "no run is active on this connection".into(),
            },
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
    // Before any thread or token: fix the descriptor ceiling child close
    // loops rely on, and let only the child tables reap their children.
    let ceiling = FdCeiling::establish().context("cannot fix a descriptor ceiling")?;
    // SAFETY: restores the default SIGCHLD action before any thread exists.
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_DFL) };
    let token_path = options.token_file.unwrap_or_else(default_token_path);
    let token = load_checked(&token_path, options.dev_same_user)?;
    let bws_path = resolve_trusted_bws(options.bws_path, options.dev_same_user)?;
    let backend: Arc<dyn VaultBackend> = Arc::new(BwsBackend::new(bws_path, token));
    backend
        .catalog(&RequestControl::detached())
        .context("initial catalog build failed")?;
    let audit_path = options
        .audit_log
        .unwrap_or_else(super::audit::default_audit_path);
    let audit_raw = FileAuditLog::open(&audit_path)?;
    let audit: Arc<dyn AuditLog> = Arc::new(audit_raw);
    let policy_path = options.policy.unwrap_or_else(default_policy_path);
    let run = Arc::new(RunConfig {
        policy: Policy::load(&policy_path, options.dev_same_user)?,
        ..RunConfig::default()
    });
    let auth = super::approve::platform_authenticator();
    let human = Arc::new(HumanConfig::serving(auth, ceiling, MAX_CONNECTIONS));
    let listener = super::server::bind_listener(&options.socket)?;
    eprintln!("hush daemon listening on {}", options.socket.display());
    super::server::run_server(listener, backend, audit, ServeConfig::default(), run, human)
}
