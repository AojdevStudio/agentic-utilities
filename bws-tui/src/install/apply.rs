//! The one checked apply owner. It consumes the plan once, observes again
//! before the first write, reads the token only after every known refusal,
//! rechecks each step's own fact right before acting, and stops at the first
//! failure or changed fact. Completed steps stay. Nothing is rolled back.

use super::custody::{Dir, FileState};
use super::facts::{Fact, Service};
use super::host::{Action, Host};
use super::input::TokenSource;
use super::layout::{rel, Backend, Platform};
use super::observe::{inspect, observe, Account, Creation, Observation, Spec};
use super::plan::{Plan, Refusal, Step};
use super::{accounts, prereq, publish};
use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::io::Write;
use std::path::Path;

/// Where apply stopped. Completed steps stay in place. Nothing is rolled back.
#[derive(Debug)]
pub(crate) struct Failure {
    pub done: Vec<Step>,
    pub step: String,
    pub error: anyhow::Error,
}

pub(crate) fn apply(
    plan: Plan,
    host: &mut dyn Host,
    token: &mut dyn TokenSource,
) -> Result<Vec<Step>, Failure> {
    let stop = |step: &str, error| Failure {
        done: Vec::new(),
        step: step.into(),
        error,
    };
    let Plan {
        observation,
        steps: planned,
    } = plan;
    let fresh = match Plan::from_observation(observe(observation.layout, host)) {
        Ok(fresh) => fresh,
        Err(Refusal(reasons)) => {
            return Err(stop(
                "recheck the observed state",
                anyhow!(reasons.join("; ")),
            ))
        }
    };
    if fresh.steps != planned {
        return Err(stop(
            "recheck the observed state",
            anyhow!("the host changed after it was observed"),
        ));
    }
    let needs_token = fresh.needs_token();
    let Plan { observation, steps } = fresh;
    if steps.is_empty() {
        return Ok(steps);
    }
    let secret = match needs_token {
        true => Some(
            token
                .read()
                .map_err(|error| stop("read the token", error))?,
        ),
        false => None,
    };
    let root = match Dir::open_root(&observation.layout.root, observation.layout.owner) {
        Fact::Known(root) => root,
        Fact::Refused(why) | Fact::Unknown(why) => {
            return Err(stop("open the install root", anyhow!(why)))
        }
    };
    let mut done = Vec::new();
    for step in steps {
        if let Err(error) = perform(&step, &observation, &root, host, secret.as_ref()) {
            return Err(Failure {
                done,
                step: step.to_string(),
                error,
            });
        }
        done.push(step);
    }
    Ok(done)
}

fn perform(
    step: &Step,
    o: &Observation,
    root: &Dir,
    host: &mut dyn Host,
    secret: Option<&SecretString>,
) -> Result<()> {
    let layout = &o.layout;
    let linux = layout.platform == Platform::Linux;
    match step {
        Step::CreateDir(path) => {
            let mode = match expect(o, root, path, FileState::Absent)? {
                Spec::Dir(mode) => *mode,
                _ => bail!("{} is not a directory artifact", path.display()),
            };
            let (parent, name) = parent(root, o, path)?;
            publish::create_dir(&parent, name, mode)
        }
        Step::CreateAccount(creation) => {
            if accounts::observe(layout, root, host)
                != Fact::Known(Account::Absent(creation.clone()))
            {
                bail!("the hushrun account changed after it was observed");
            }
            host.act(&match creation {
                Creation::Linux { gid, shell } => Action::CreateLinuxAccount {
                    gid: *gid,
                    shell: shell.clone(),
                },
                Creation::Mac { uid, nobody_gid } => Action::CreateMacAccount {
                    uid: *uid,
                    nobody_gid: *nobody_gid,
                    scratch: layout.host_path(Path::new(layout.token_dir())),
                },
            })?;
            match accounts::observe(layout, root, host) {
                Fact::Known(Account::Ready(_)) => Ok(()),
                _ => bail!("the new hushrun account did not read back as created"),
            }
        }
        Step::CreateHostKey => {
            let path = o
                .artifacts
                .iter()
                .find(|(artifact, _)| artifact.spec == Spec::HostKey)
                .map(|(artifact, _)| artifact.path.clone())
                .context("no host key artifact")?;
            expect(o, root, &path, FileState::Absent)?;
            host.act(&Action::CreateHostKey)?;
            expect(o, root, &path, FileState::Ready)
                .map(drop)
                .context("the new host key did not read back as private")
        }
        Step::FinishStage(path, stage) => {
            expect(
                o,
                root,
                path,
                FileState::Resume {
                    stage: stage.clone(),
                },
            )?;
            let (parent, name) = parent(root, o, path)?;
            publish::finish_stage(&parent, name, stage)
        }
        Step::StoreCredential(path) => {
            expect(o, root, path, FileState::Absent)?;
            let secret = secret.context("no token was read")?;
            let encrypted = matches!(o.backend, Fact::Known(Backend::Encrypted));
            let (parent, name) = parent(root, o, path)?;
            publish::publish(&parent, name, 0o600, |file| match encrypted {
                true => host.encrypt(secret, file),
                false => Ok(file.write_all(secret.expose_secret().as_bytes())?),
            })
        }
        Step::Publish(path) => {
            let (bytes, mode) = match expect(o, root, path, FileState::Absent)? {
                Spec::Rendered(bytes, mode) => (bytes.clone(), *mode),
                Spec::Pam(Some(bytes)) => (bytes.clone(), 0o644),
                _ => bail!("{} has nothing to write", path.display()),
            };
            let (parent, name) = parent(root, o, path)?;
            publish::publish(&parent, name, mode, |file| Ok(file.write_all(&bytes)?))
        }
        Step::Reload => manage(
            o,
            host,
            |s| !s.loaded || s.needs_reload,
            Action::DaemonReload,
            |s| s.loaded && !s.needs_reload,
        ),
        Step::Load => manage(
            o,
            host,
            |s| !s.loaded,
            Action::LaunchdBootstrap,
            |s| s.loaded,
        ),
        Step::Enable => manage(
            o,
            host,
            |s| !s.enabled,
            if linux {
                Action::EnableUnit
            } else {
                Action::LaunchdEnable
            },
            |s| s.enabled,
        ),
        Step::Start => manage(
            o,
            host,
            |s| !s.active,
            if linux {
                Action::StartUnit
            } else {
                Action::LaunchdKickstart
            },
            |s| s.active,
        ),
    }
}

/// The artifact at `path`, after proving its current state is still `state`.
fn expect<'o>(o: &'o Observation, root: &Dir, path: &Path, state: FileState) -> Result<&'o Spec> {
    let artifact = o
        .artifacts
        .iter()
        .map(|(artifact, _)| artifact)
        .find(|artifact| artifact.path == path)
        .with_context(|| format!("{} is not an install artifact", path.display()))?;
    if inspect(root, &o.layout, artifact) != Fact::Known(state) {
        bail!("{} changed after it was observed", path.display());
    }
    Ok(&artifact.spec)
}

fn parent<'p>(root: &Dir, o: &Observation, path: &'p Path) -> Result<(Dir, &'p str)> {
    let (Some(parent), Some(name)) = (
        path.parent(),
        path.file_name().and_then(|name| name.to_str()),
    ) else {
        bail!("{} names no file", path.display());
    };
    match root.walk(rel(parent), o.layout.owner) {
        Fact::Known(super::custody::Walk::Found(dir)) => Ok((dir, name)),
        _ => bail!("{} is no longer a checked directory", parent.display()),
    }
}

/// One manager operation: recheck that it is still needed, act once, and
/// read back that it took effect. No retry.
fn manage(
    o: &Observation,
    host: &mut dyn Host,
    needed: fn(&Service) -> bool,
    action: Action,
    effect: fn(&Service) -> bool,
) -> Result<()> {
    let bws = match &o.bws {
        Fact::Known(bws) => Some(bws.as_path()),
        _ => None,
    };
    let before = prereq::service(&o.layout, host, bws);
    if !matches!(&before, Fact::Known(service) if needed(service)) {
        bail!("the service manager state changed after it was observed");
    }
    host.act(&action)?;
    match prereq::service(&o.layout, host, bws) {
        Fact::Known(service) if effect(&service) => Ok(()),
        _ => bail!("the service manager did not report the expected state afterwards"),
    }
}
