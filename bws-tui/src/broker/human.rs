//! Daemon handling of human operations. Proposed keys and notes are checked
//! first, then descriptors against the operation's roles. Reveal, copy, edit,
//! and delete then need a fresh password approval on the checked terminal.
//! Add needs none and never overwrites. Values reach only the terminal or the
//! vault. One request control, made at entry, bounds the whole request and
//! is checked between every stage, so a client that leaves gets no
//! disclosure and no write. The native password check itself cannot be
//! interrupted, so it is checked before and after, not bounded.

use super::approve::Authenticator;
use super::approve::APPROVAL_TIMEOUT;
use super::approve::{approve, terminal_refusal, ApprovalRequest};
use super::audit::{AuditLog, HumanAudit};
use super::backend::VaultBackend;
use super::children::Children;
use super::console::{escape, MAX_OSC52_VALUE};
use super::fdpass::RightsViolation;
use super::fields::{check_proposed, invalid};
use super::meta::SecretRef;
use super::mutate::{add, delete, edit, live, read_failed, resolve, resolve_project};
use super::operation::{DescriptorProblem, Operation, Outcome, Refusal, SecretSelector};
use super::operation::{ValueChange, ValueSource};
use super::peercred::PeerCredentials;
use super::protocol::ResponseFrame;
use super::request::RequestControl;
use super::slots::{descriptors, Readers, Slots};
use super::spawn_fd::FdCeiling;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Shared by every connection. The mutation lock serializes daemon writes.
/// The reader table bounds value-input children across all connections.
pub(crate) struct HumanConfig {
    pub(crate) auth: Arc<dyn Authenticator>,
    pub(crate) mutation: Mutex<()>,
    /// The whole request's one deadline, from handler entry.
    pub(crate) timeout: Duration,
    readers: Children,
    ceiling: FdCeiling,
}

impl HumanConfig {
    /// `ceiling` must be fixed at daemon startup, before any thread exists.
    pub(crate) fn serving(
        auth: Arc<dyn Authenticator>,
        ceiling: FdCeiling,
        readers: usize,
    ) -> Self {
        Self {
            auth,
            mutation: Mutex::new(()),
            timeout: APPROVAL_TIMEOUT,
            readers: Children::new(readers),
            ceiling,
        }
    }

    #[cfg(test)]
    pub(crate) fn new(auth: Arc<dyn Authenticator>) -> Self {
        let ceiling = FdCeiling::establish().expect("test descriptor ceiling");
        Self::serving(auth, ceiling, super::daemon::MAX_CONNECTIONS)
    }
}

#[cfg(test)]
impl Default for HumanConfig {
    fn default() -> Self {
        Self::new(super::approve::platform_authenticator())
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

/// What a reveal, copy, delete, or edit does once the human has approved it.
enum Action {
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
    control: &'a RequestControl<'a>,
    peer: PeerCredentials,
    verb: &'static str,
    slots: Slots<'a>,
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
    check_proposed(&op)?;
    let control = RequestControl::until(socket, config.timeout);
    live(&control)?;
    let readers = Readers {
        children: &config.readers,
        ceiling: config.ceiling,
    };
    let mut context = Context {
        backend,
        audit,
        config,
        control: &control,
        peer,
        verb: op.verb(),
        slots: Slots::bind(op.roles(), fds, peer, &control, readers)?,
    };
    match op {
        Operation::Add {
            project,
            key,
            note,
            value,
        } => add_secret(&mut context, &project, key, note, value),
        Operation::Reveal { secret } => approved(&mut context, &secret, Action::Reveal),
        Operation::Copy { secret } => approved(&mut context, &secret, Action::Copy),
        Operation::Delete { secret } => approved(&mut context, &secret, Action::Delete),
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
            let edit = Action::Edit {
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
    let control = context.control;
    let project = resolve_project(context.backend, project, control)?;
    context.record(&key, Some(&project.name), None)?;
    live(control)?;
    let prompt = format!("Value for {} (input hidden): ", escape(key.as_bytes()));
    let value = context.slots.read_value(source, &prompt)?;
    live(control)?;
    let lock = &context.config.mutation;
    add(context.backend, lock, control, &project, key, note, &value)
}

/// Resolve, audit, approve on the terminal, then act.
fn approved(
    context: &mut Context<'_>,
    secret: &SecretSelector,
    action: Action,
) -> Result<Outcome, Refusal> {
    let (backend, verb, peer) = (context.backend, context.verb, context.peer);
    let control = context.control;
    let meta = resolve(backend, secret, verb, control)?;
    context.record(&meta.key, meta.project.as_deref(), Some(&meta.id))?;
    live(control)?;
    let request = ApprovalRequest {
        verb,
        key: &meta.key,
        project: meta.project.as_deref(),
        id: &meta.id,
        uid: peer.uid,
        pid: peer.pid,
    };
    approve(context.slots.console()?, &request, &*context.config.auth)?;
    live(control)?;
    match action {
        Action::Reveal | Action::Copy => {
            let value = backend
                .fetch(&SecretRef::Id(meta.id.clone()), control)
                .map_err(|_| read_failed(control, verb))?;
            live(control)?;
            let console = context.slots.console()?;
            if matches!(action, Action::Copy) {
                if value.expose_bytes().len() > MAX_OSC52_VALUE {
                    return Err(invalid(
                        "the value is larger than the 128 KiB clipboard limit",
                    ));
                }
                console
                    .osc52(value.expose_bytes())
                    .map_err(terminal_refusal)?;
                return Ok(Outcome::CopySent { key: meta.key });
            }
            let project = meta.project.as_deref().unwrap_or("no project");
            let title = format!("{} ({project})", meta.key);
            console
                .show_value(&title, value.expose_bytes())
                .map_err(terminal_refusal)?;
            Ok(Outcome::Revealed { key: meta.key })
        }
        Action::Delete => delete(backend, &context.config.mutation, control, &meta),
        Action::Edit {
            new_key,
            note,
            value,
        } => {
            let value = match value {
                ValueChange::Keep => None,
                ValueChange::Replace(source) => {
                    let name = escape(meta.key.as_bytes());
                    let prompt = format!("New value for {name} (input hidden): ");
                    Some(context.slots.read_value(source, &prompt)?)
                }
            };
            live(control)?;
            let lock = &context.config.mutation;
            edit(backend, lock, control, &meta, new_key, note, value)
        }
    }
}
