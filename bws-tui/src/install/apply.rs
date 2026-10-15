//! The one owner that applies a plan. It reads the token, if one is needed,
//! after every known refusal and before the first durable write. Before each
//! step it observes again and stops at the first refusal, change, or error.
//! Completed steps stay in place. Nothing is rolled back.

use super::host::Host;
use super::layout::{Kind, Layout};
use super::model::Backend;
use super::observe::{observe, Binaries};
use super::plan::{Plan, Refusal, Step};
use super::publish::{create_dir, publish};
use anyhow::{bail, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::Write;

/// Supplies the token once, only when a plan writes one.
pub(crate) trait TokenSource {
    fn read_token(&mut self) -> Result<SecretString>;
}

#[derive(Debug)]
pub(crate) enum Halt {
    /// Known before input or any write.
    Refused(Vec<Refusal>),
    /// The host changed after the plan, so later steps did not run.
    Changed { completed: Vec<String>, refusals: Vec<Refusal> },
    Failed { completed: Vec<String>, step: String, error: anyhow::Error },
}

pub(crate) fn install_with(
    layout: &Layout,
    binaries: &Binaries,
    host: &mut dyn Host,
    input: &mut dyn TokenSource,
) -> Result<Vec<String>, Halt> {
    let plan = Plan::from_observed(&observe(layout, binaries, host)).map_err(Halt::Refused)?;
    let token = if plan.needs_token() {
        let read = input.read_token().map_err(|error| Halt::Failed {
            completed: Vec::new(),
            step: "read the token".into(),
            error,
        })?;
        Some(read)
    } else {
        None
    };
    let mut remaining = plan.steps;
    let mut completed = Vec::new();
    loop {
        let fresh = match Plan::from_observed(&observe(layout, binaries, host)) {
            Ok(fresh) if fresh.fits_within(&remaining) => fresh,
            Ok(_) => return Err(Halt::Changed { completed, refusals: Vec::new() }),
            Err(refusals) => return Err(Halt::Changed { completed, refusals }),
        };
        remaining = fresh.steps;
        let Some(step) = remaining.first().cloned() else {
            return Ok(completed);
        };
        if let Err(error) = execute(&step, layout, host, token.as_ref()) {
            return Err(Halt::Failed { completed, step: step.to_string(), error });
        }
        completed.push(step.to_string());
        remaining.remove(0);
    }
}

fn execute(step: &Step, layout: &Layout, host: &mut dyn Host, token: Option<&SecretString>) -> Result<()> {
    match step {
        Step::CreateDir(path) => create_dir(layout, path, Kind::Dir.mode()),
        Step::Native(operation) => host.perform(operation),
        Step::WriteFile(spec) => publish(layout, spec, |file| match &spec.kind {
            Kind::File(bytes) | Kind::Pam(bytes) => Ok(file.write_all(bytes)?),
            _ => bail!("only rendered files are written this way"),
        }),
        Step::WriteToken(spec) => {
            let Some(token) = token else {
                bail!("the token was not read before its step");
            };
            publish(layout, spec, |file| match spec.kind {
                Kind::Token(Backend::Encrypted) => host.encrypt(token, file),
                _ => {
                    file.write_all(token.expose_secret().as_bytes())
                        .map_err(|_| anyhow::anyhow!("could not write the token"))?;
                    Ok(())
                }
            })
        }
    }
}
