//! The private install plan. It is a pure function of one observation, so
//! every known refusal is found before input or any durable write.

use super::host::Operation;
use super::layout::{Kind, Spec};
use super::model::{Backend, Fact, Presence, Socket};
use super::observe::Observed;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    CreateDir(PathBuf),
    Native(Operation),
    WriteToken(Spec),
    WriteFile(Spec),
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir(path) => write!(f, "create {}", path.display()),
            Self::Native(operation) => write!(f, "{operation}"),
            Self::WriteToken(spec) | Self::WriteFile(spec) => write!(f, "write {}", spec.path.display()),
        }
    }
}

/// One reason install will not start. `unknown` means it could not be observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub subject: String,
    pub reason: &'static str,
    pub unknown: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub steps: Vec<Step>,
}

struct Gate(Vec<Refusal>);

impl Gate {
    fn take<T: Clone>(&mut self, subject: impl Into<String>, fact: &Fact<T>) -> Option<T> {
        let (reason, unknown) = match fact {
            Fact::Known(value) => return Some(value.clone()),
            Fact::Refused(reason) => (*reason, false),
            Fact::Unknown(reason) => (*reason, true),
        };
        self.0.push(Refusal { subject: subject.into(), reason, unknown });
        None
    }

    fn refuse(&mut self, subject: &str, reason: &'static str) {
        self.0.push(Refusal { subject: subject.into(), reason, unknown: false });
    }
}

impl Plan {
    pub(crate) fn needs_token(&self) -> bool {
        self.steps.iter().any(|step| matches!(step, Step::WriteToken(_)))
    }

    /// Every refusal at once, or the steps that remain. Unknown refuses too.
    pub(crate) fn from_observed(observed: &Observed) -> Result<Self, Vec<Refusal>> {
        let mut gate = Gate(Vec::new());
        gate.take("service executable", &observed.hush);
        gate.take("bws executable", &observed.bws);
        let backend = gate.take("token backend", &observed.backend);
        gate.take("service files", &observed.rendered);
        let host_key = gate.take("systemd host key", &observed.host_key);
        let request = gate.take("hushrun account", &observed.request);
        let account = gate.take("hushrun account", &observed.account);
        let manager = gate.take("hush service", &observed.manager);
        let socket = gate.take("broker socket", &observed.socket);
        let entries: Vec<(Spec, Option<Presence>)> = observed
            .entries
            .iter()
            .map(|entry| (entry.spec.clone(), gate.take(entry.spec.path.display().to_string(), &entry.presence)))
            .collect();
        let (Some(backend), Some(host_key), Some(request), Some(account), Some(manager), Some(socket)) =
            (backend, host_key, request, account, manager, socket)
        else {
            return Err(gate.0);
        };
        if !gate.0.is_empty() {
            return Err(gate.0);
        }
        if socket == Socket::Foreign {
            gate.refuse("broker socket", "another process answers on the broker socket");
        }
        let absent = |kind: fn(&Kind) -> bool| {
            entries.iter().filter(move |(spec, presence)| kind(&spec.kind) && *presence == Some(Presence::Absent))
        };
        let token_absent = absent(|kind| matches!(kind, Kind::Token(_))).next().is_some();
        if backend == Backend::Encrypted && host_key == Presence::Absent && !token_absent {
            gate.refuse("systemd host key", "an encrypted token exists, but the host key that decrypts it is missing");
        }
        let definition_new = absent(|kind| matches!(kind, Kind::File(_))).next().is_some();
        if definition_new && manager.loaded {
            gate.refuse("hush service", "the manager has a hush service install did not write");
        }
        if !gate.0.is_empty() {
            return Err(gate.0);
        }
        let mut steps: Vec<Step> = absent(|kind| *kind == Kind::Dir)
            .map(|(spec, _)| Step::CreateDir(spec.path.clone()))
            .collect();
        if account == Presence::Absent {
            steps.push(Step::Native(Operation::CreateAccount(request)));
        }
        if backend == Backend::Encrypted && host_key == Presence::Absent {
            steps.push(Step::Native(Operation::CreateHostKey));
        }
        steps.extend(absent(|kind| matches!(kind, Kind::Token(_))).map(|(spec, _)| Step::WriteToken(spec.clone())));
        steps.extend(absent(|kind| matches!(kind, Kind::File(_) | Kind::Pam(_))).map(|(spec, _)| Step::WriteFile(spec.clone())));
        let register = definition_new || !manager.loaded || manager.stale;
        let enable = definition_new || !manager.enabled;
        let start = definition_new || manager.pid.is_none();
        let service = if cfg!(target_os = "macos") {
            [(enable, Operation::Enable), (register, Operation::Register), (start, Operation::Start)]
        } else {
            [(register, Operation::Register), (enable, Operation::Enable), (start, Operation::Start)]
        };
        steps.extend(service.into_iter().filter(|(wanted, _)| *wanted).map(|(_, op)| Step::Native(op)));
        Ok(Self { steps })
    }

    /// A recheck may find steps already done as a side effect, but never a
    /// new step. Anything else changed after the plan was made.
    pub(crate) fn fits_within(&self, remaining: &[Step]) -> bool {
        let mut rest = remaining.iter();
        self.steps.iter().all(|step| rest.any(|left| left == step))
    }
}
