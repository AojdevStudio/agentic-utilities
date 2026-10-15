//! Preserve reported daemon outcomes when a late stdin send fails.

use crate::broker::runspec::RunOutcome;
use anyhow::{bail, Result};
use std::sync::Mutex;

pub(super) enum ReportedRun {
    Exit(RunOutcome),
    Refused(String),
}

/// Why the stdin worker stopped early.
pub(super) enum InputFailure {
    /// The child may have taken the partial input as complete, so this
    /// outranks even a reported exit.
    Read(std::io::Error),
    /// The connection broke. A reported daemon reply still stands.
    Send(anyhow::Error),
}

/// Settles a run once: the daemon's reply against a stdin worker failure.
#[derive(Default)]
pub(super) struct Completion(Mutex<Settled>);

#[derive(Default)]
enum Settled {
    #[default]
    Running,
    Failed(InputFailure),
    Done,
}

impl Completion {
    pub(super) fn fail(&self, failure: InputFailure) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*state, Settled::Running) {
            *state = Settled::Failed(failure);
        }
    }

    pub(super) fn settle(&self, result: Result<ReportedRun>) -> Result<RunOutcome> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let reply = match std::mem::replace(&mut *state, Settled::Done) {
            Settled::Failed(InputFailure::Read(error)) => {
                return Err(anyhow::Error::new(error).context("reading stdin for the run failed"));
            }
            Settled::Failed(InputFailure::Send(error)) if result.is_err() => {
                return Err(error.context("sending stdin to the daemon failed"));
            }
            _ => result?,
        };
        match reply {
            ReportedRun::Exit(outcome) => Ok(outcome),
            ReportedRun::Refused(message) => bail!("{message}"),
        }
    }
}
