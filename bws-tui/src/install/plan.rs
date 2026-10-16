//! The private plan. Every known refusal is found here, before input or writes.
//! Steps come from the artifact table and the observed account and manager
//! state, so there is no second inventory of what install does.

use super::custody::FileState;
use super::facts::Fact;
use super::layout::{Platform, HOST_KEY};
use super::observe::{Account, Creation, Observation, Spec};
use super::reach::{is_managed, Reach};
use crate::broker::identity::HUSHRUN_ACCOUNT;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    CreateDir(PathBuf),
    CreateAccount(Creation),
    CreateHostKey,
    FinishStage(PathBuf, String),
    StoreCredential(PathBuf),
    Publish(PathBuf),
    /// systemd daemon-reload.
    Reload,
    /// launchd bootstrap of the plist. RunAtLoad starts the job.
    Load,
    Enable,
    Start,
}

/// Every reason install refuses, all found before input or writes.
#[derive(Debug)]
pub(crate) struct Refusal(pub Vec<String>);

pub(crate) struct Plan {
    pub(super) observation: Observation,
    pub(super) steps: Vec<Step>,
}

impl Plan {
    pub(crate) fn from_observation(observation: Observation) -> Result<Plan, Refusal> {
        let mut reasons = Vec::new();
        let o = &observation;
        for fact in &o.checks {
            note(fact, &mut reasons);
        }
        note(&o.backend, &mut reasons);
        note(&o.bws, &mut reasons);
        for (_, fact) in &o.artifacts {
            note(fact, &mut reasons);
        }
        note(&o.account, &mut reasons);
        note(&o.service, &mut reasons);
        note(&o.socket, &mut reasons);
        if let (Fact::Known(service), Fact::Known(Reach::Peer(peer))) = (&o.service, &o.socket) {
            if !is_managed(service, peer, o.layout.owner) {
                reasons.push(format!(
                    "a process that is not the managed hush daemon listens at {}",
                    o.layout.socket.display()
                ));
            }
        }
        if !reasons.is_empty() {
            return Err(Refusal(reasons));
        }
        let steps = steps(&observation);
        Ok(Plan { observation, steps })
    }

    pub(crate) fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub(crate) fn needs_token(&self) -> bool {
        self.steps
            .iter()
            .any(|step| matches!(step, Step::StoreCredential(_)))
    }
}

fn note<T>(fact: &Fact<T>, reasons: &mut Vec<String>) {
    if let Fact::Refused(why) | Fact::Unknown(why) = fact {
        reasons.push(why.clone());
    }
}

/// Only missing or resumable work, in apply order. The account follows the
/// directories so macOS can stage its import record in the private one.
fn steps(o: &Observation) -> Vec<Step> {
    let mut steps: Vec<Step> = o
        .artifacts
        .iter()
        .filter_map(|(artifact, fact)| {
            let path = artifact.path.clone();
            match (fact, &artifact.spec) {
                (Fact::Known(FileState::Resume { stage }), _) => {
                    Some(Step::FinishStage(path, stage.clone()))
                }
                (Fact::Known(FileState::Absent), Spec::Dir(_)) => Some(Step::CreateDir(path)),
                (Fact::Known(FileState::Absent), Spec::HostKey) => Some(Step::CreateHostKey),
                (Fact::Known(FileState::Absent), Spec::Credential) => {
                    Some(Step::StoreCredential(path))
                }
                (Fact::Known(FileState::Absent), Spec::Rendered(..) | Spec::Pam(_)) => {
                    Some(Step::Publish(path))
                }
                _ => None,
            }
        })
        .collect();
    if let Fact::Known(Account::Absent(creation)) = &o.account {
        let after_dirs = steps
            .iter()
            .take_while(|step| matches!(step, Step::CreateDir(_)))
            .count();
        steps.insert(after_dirs, Step::CreateAccount(creation.clone()));
    }
    if let Fact::Known(service) = o.service {
        let wanted = match o.layout.platform {
            Platform::Linux => [
                (!service.loaded || service.needs_reload, Step::Reload),
                (!service.enabled, Step::Enable),
                (!service.active, Step::Start),
            ],
            Platform::MacOs => [
                (!service.enabled, Step::Enable),
                (!service.loaded, Step::Load),
                (service.loaded && !service.active, Step::Start),
            ],
        };
        steps.extend(
            wanted
                .into_iter()
                .filter_map(|(needed, step)| needed.then_some(step)),
        );
    }
    steps
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::CreateDir(path) => write!(f, "create directory {}", path.display()),
            Step::CreateAccount(_) => write!(f, "create account {HUSHRUN_ACCOUNT}"),
            Step::CreateHostKey => {
                write!(f, "create the systemd credential host key at {HOST_KEY}")
            }
            Step::FinishStage(path, _) => {
                write!(f, "remove the leftover staging link for {}", path.display())
            }
            Step::StoreCredential(path) => write!(f, "store the token at {}", path.display()),
            Step::Publish(path) => write!(f, "write {}", path.display()),
            Step::Reload => write!(f, "reload systemd"),
            Step::Load => write!(f, "load the launchd job"),
            Step::Enable => write!(f, "enable hush at boot"),
            Step::Start => write!(f, "start hush"),
        }
    }
}
