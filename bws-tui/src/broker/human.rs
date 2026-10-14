//! Daemon handling of human operations. Descriptors are checked against the
//! operation's roles first. Reveal, copy, edit, and delete then need a fresh
//! password approval on the checked terminal. Add needs none and never
//! overwrites. Values reach only the terminal or the vault.

use super::approve::{approve, platform_authenticator, ApprovalRequest, Authenticator};
use super::audit::{AuditLog, HumanAudit};
use super::backend::VaultBackend;
use super::console::{escape, Interrupt, MAX_OSC52_VALUE};
use super::fdpass::RightsViolation;
use super::meta::SecretRef;
use super::mutate::{add, edit, invalid, resolve, resolve_project, vault_failed};
use super::operation::{DescriptorProblem, Operation, Outcome, Refusal, SecretSelector};
use super::operation::{ValueChange, ValueSource};
use super::peercred::PeerCredentials;
use super::protocol::ResponseFrame;
use super::slots::{descriptors, Slots};
use super::tty::check_terminal;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

/// Shared by every connection. The mutation lock serializes daemon writes.
pub(crate) struct HumanConfig {
    pub(crate) auth: Arc<dyn Authenticator>,
    pub(crate) mutation: Mutex<()>,
}

impl HumanConfig {
    pub(crate) fn new(auth: Arc<dyn Authenticator>) -> Self {
        Self {
            auth,
            mutation: Mutex::new(()),
        }
    }
}

impl Default for HumanConfig {
    fn default() -> Self {
        Self::new(platform_authenticator())
    }
}

/// One human request and the connection it arrived on.
pub(crate) struct HumanRequest<'a> {
    pub op: Operation,
    pub fds: Vec<OwnedFd>,
    pub violation: Option<RightsViolation>,
    pub peer: PeerCredentials,
    pub socket: &'a UnixStream,
}

pub(crate) fn serve_human(
    request: HumanRequest<'_>,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    config: &HumanConfig,
) -> ResponseFrame {
    match handle(request, backend, audit, config) {
        Ok(outcome) => ResponseFrame::Done { outcome },
        Err(refusal) => ResponseFrame::Refused { refusal },
    }
}

/// What an approved operation does once the human has approved it.
enum Approved {
    Reveal,
    Copy,
    Delete,
    Edit {
        new_key: Option<String>,
        note: Option<String>,
        value: ValueChange,
    },
}

/// One request's checked descriptors and the services it may use.
struct Context<'a> {
    backend: &'a dyn VaultBackend,
    audit: &'a dyn AuditLog,
    config: &'a HumanConfig,
    peer: PeerCredentials,
    socket: &'a UnixStream,
    verb: &'static str,
    slots: Slots,
}

impl Context<'_> {
    fn record(&self, key: &str, project: Option<&str>, id: Option<&str>) -> Result<(), Refusal> {
        let line = HumanAudit {
            verb: self.verb,
            key,
            project,
            id,
        };
        self.audit
            .record_human(self.peer, &line)
            .map_err(|_| Refusal::AuditFailed)
    }
}

fn handle(
    request: HumanRequest<'_>,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    config: &HumanConfig,
) -> Result<Outcome, Refusal> {
    let HumanRequest {
        op,
        fds,
        violation,
        peer,
        socket,
    } = request;
    if violation.is_some() {
        return Err(descriptors(DescriptorProblem::Malformed));
    }
    let mut context = Context {
        backend,
        audit,
        config,
        peer,
        socket,
        verb: op.verb(),
        slots: Slots::bind(op.contract().roles, fds, peer)?,
    };
    match op {
        Operation::Add {
            project,
            key,
            note,
            value,
        } => add_secret(&mut context, &project, key, note, value),
        Operation::Reveal { secret } => approved(&mut context, &secret, Approved::Reveal),
        Operation::Copy { secret } => approved(&mut context, &secret, Approved::Copy),
        Operation::Delete { secret } => approved(&mut context, &secret, Approved::Delete),
        Operation::Edit {
            new_key: None,
            note: None,
            value: ValueChange::Keep,
            ..
        } => Err(Refusal::NothingToChange),
        Operation::Edit {
            secret,
            new_key,
            note,
            value,
        } => {
            let edit = Approved::Edit {
                new_key,
                note,
                value,
            };
            approved(&mut context, &secret, edit)
        }
    }
}

/// No approval. The value comes from the passed input or a hidden prompt.
fn add_secret(
    context: &mut Context<'_>,
    project: &str,
    key: String,
    note: Option<String>,
    source: ValueSource,
) -> Result<Outcome, Refusal> {
    let project = resolve_project(context.backend, project)?;
    context.record(&key, Some(&project.name), None)?;
    let prompt = format!("Value for {} (input hidden): ", escape(key.as_bytes()));
    let value = context.slots.read_value(source, &prompt, context.socket)?;
    add(
        context.backend,
        &context.config.mutation,
        &project,
        key,
        note,
        &value,
    )
}

/// Resolve, audit, approve on the terminal, then act.
fn approved(
    context: &mut Context<'_>,
    secret: &SecretSelector,
    action: Approved,
) -> Result<Outcome, Refusal> {
    let (backend, verb, peer, socket) =
        (context.backend, context.verb, context.peer, context.socket);
    let meta = resolve(backend, secret, verb)?;
    context.record(&meta.key, meta.project.as_deref(), Some(&meta.id))?;
    let request = ApprovalRequest {
        verb,
        key: &meta.key,
        project: meta.project.as_deref(),
        id: &meta.id,
        uid: peer.uid,
        pid: peer.pid,
    };
    approve(
        context.slots.console()?,
        &request,
        &*context.config.auth,
        socket,
    )?;
    match action {
        Approved::Reveal | Approved::Copy => {
            let value = backend
                .fetch(&SecretRef::Id(meta.id.clone()))
                .map_err(|_| vault_failed(verb))?;
            let console = context.slots.console()?;
            check_terminal(console.fd(), peer).map_err(descriptors)?;
            if matches!(action, Approved::Copy) {
                if value.expose_bytes().len() > MAX_OSC52_VALUE {
                    return Err(invalid(
                        "the value is larger than the 128 KiB clipboard limit",
                    ));
                }
                console
                    .osc52(value.expose_bytes())
                    .map_err(|_| Refusal::TerminalFailed)?;
                return Ok(Outcome::CopySent { key: meta.key });
            }
            let project = meta.project.as_deref().unwrap_or("no project");
            let title = format!("{} ({project})", meta.key);
            match console.show_value(&title, value.expose_bytes(), socket) {
                Ok(()) => Ok(Outcome::Revealed { key: meta.key }),
                Err(Interrupt::Disconnected | Interrupt::TimedOut) => {
                    Err(Refusal::ApprovalCancelled)
                }
                Err(_) => Err(Refusal::TerminalFailed),
            }
        }
        Approved::Delete => {
            backend.delete(&meta.id).map_err(|_| vault_failed(verb))?;
            Ok(Outcome::Deleted {
                id: meta.id,
                key: meta.key,
            })
        }
        Approved::Edit {
            new_key,
            note,
            value,
        } => {
            let value = match value {
                ValueChange::Keep => None,
                ValueChange::Replace(source) => {
                    let name = escape(meta.key.as_bytes());
                    let prompt = format!("New value for {name} (input hidden): ");
                    Some(context.slots.read_value(source, &prompt, socket)?)
                }
            };
            let lock = &context.config.mutation;
            edit(backend, lock, &meta, new_key, note, value)
        }
    }
}
