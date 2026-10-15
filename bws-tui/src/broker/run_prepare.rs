//! Daemon run steps 1 to 5 from ADR 0002: resolve, audit, pick the identity,
//! fetch, and build the child environment from scratch.

use super::audit::{AuditLog, RunAudit};
use super::backend::VaultBackend;
use super::identity::{resolve_identity, AccountDb, Identity, SystemAccounts};
use super::mask::MIN_MASKED_LEN;
use super::meta::{SecretMeta, SecretRef};
use super::peercred::PeerCredentials;
use super::policy::Policy;
use super::protocol::{WireBinding, WireRunSpec};
use super::run_program::program_candidates;
use super::runspec::{child_var, is_forbidden_env, is_passthrough_allowed, Delivery};
use super::spawn::{ChildPlan, FIRST_DELIVERY_FD, MAX_DELIVERY_FDS};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::ffi::{CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;
use zeroize::Zeroizing;

pub(crate) struct RunConfig {
    pub policy: Policy,
    pub accounts: Arc<dyn AccountDb>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            policy: Policy::default(),
            accounts: Arc::new(SystemAccounts),
        }
    }
}

/// Every value buffer here zeroes on drop.
pub(crate) struct PreparedRun {
    pub plan: ChildPlan,
    pub fd_values: Vec<Zeroizing<Vec<u8>>>,
    pub mask_values: Vec<Zeroizing<Vec<u8>>>,
    pub short_keys: Vec<String>,
}

pub(crate) fn prepare_run(
    peer: PeerCredentials,
    spec: WireRunSpec,
    backend: &dyn VaultBackend,
    audit: &dyn AuditLog,
    config: &RunConfig,
) -> Result<PreparedRun> {
    check_bindings(&spec.bindings)?;
    let catalog = backend.catalog()?;
    let resolved: Vec<Result<&SecretMeta>> = spec
        .bindings
        .iter()
        .map(|binding| resolve_key(&catalog, binding))
        .collect();
    let run_as = config
        .policy
        .run_as_for(spec.bindings.iter().map(|binding| binding.key.as_str()));
    let program = OsStr::from_bytes(&spec.program.0);
    audit.record_run(
        peer,
        &RunAudit {
            program: &program.to_string_lossy(),
            run_as,
            secrets: spec
                .bindings
                .iter()
                .zip(&resolved)
                .map(|(binding, meta)| {
                    (
                        binding.key.as_str(),
                        meta.as_ref().ok().map(|m| m.id.as_str()),
                    )
                })
                .collect(),
        },
    )?;
    let ids = resolved.into_iter().collect::<Result<Vec<_>>>()?;
    let identity = resolve_identity(peer, run_as, &*config.accounts)?;
    let cwd = PathBuf::from(OsStr::from_bytes(&spec.cwd.0));
    if !cwd.is_absolute() || !cwd.is_dir() {
        bail!("working directory does not exist: {}", cwd.display());
    }
    let path_var = spec
        .env
        .iter()
        .find(|(name, _)| name.as_str() == "PATH")
        .map(|(_, value)| value.0.as_slice());
    let programs = program_candidates(program, &cwd, path_var)?;
    let switch = identity_switch(identity)?;

    let mut bound = Vec::with_capacity(spec.bindings.len());
    let mut fd_values = Vec::new();
    let mut mask_values = Vec::with_capacity(spec.bindings.len());
    let mut short_keys = Vec::new();
    for (binding, meta) in spec.bindings.iter().zip(ids) {
        let material = backend.fetch(&SecretRef::Id(meta.id.clone()))?;
        let value = Zeroizing::new(material.expose_bytes().to_vec());
        drop(material);
        if value.len() < MIN_MASKED_LEN && !short_keys.contains(&binding.key) {
            short_keys.push(binding.key.clone());
        }
        mask_values.push(value.clone());
        match binding.delivery {
            Delivery::Env => {
                if value.contains(&0) {
                    bail!(
                        "{} holds a NUL byte and cannot go in the environment. Use --fd",
                        binding.key
                    );
                }
                bound.push((binding.env.as_str().to_owned(), value));
            }
            Delivery::Fd => {
                let fd = FIRST_DELIVERY_FD + fd_values.len() as i32;
                bound.push((
                    child_var(&binding.env, Delivery::Fd),
                    Zeroizing::new(fd.to_string().into_bytes()),
                ));
                fd_values.push(value);
            }
        }
    }
    let passthrough: Vec<(&str, &[u8])> = spec
        .env
        .iter()
        .map(|(name, value)| (name.as_str(), value.0.as_slice()))
        .collect();
    let mut argv = vec![CString::new(spec.program.0.clone()).context("program holds a NUL byte")?];
    for arg in &spec.args {
        argv.push(CString::new(arg.0.clone()).context("argument holds a NUL byte")?);
    }
    Ok(PreparedRun {
        plan: ChildPlan {
            programs,
            argv,
            envp: child_env(&passthrough, bound),
            cwd: CString::new(spec.cwd.0)?,
            identity: switch,
            delivery_fds: fd_values.len(),
        },
        fd_values,
        mask_values,
        short_keys,
    })
}

/// Build `NAME=value\0` entries: allowlisted passthrough, then bindings over it.
/// Forbidden prefixes never survive, whichever side named them.
pub(crate) fn child_env(
    passthrough: &[(&str, &[u8])],
    bound: Vec<(String, Zeroizing<Vec<u8>>)>,
) -> Vec<Zeroizing<Vec<u8>>> {
    let mut env: BTreeMap<String, Zeroizing<Vec<u8>>> = BTreeMap::new();
    for (name, value) in passthrough {
        if is_passthrough_allowed(name) && !value.contains(&0) {
            env.insert((*name).to_owned(), Zeroizing::new(value.to_vec()));
        }
    }
    for (name, value) in bound {
        if !is_forbidden_env(&name) {
            env.insert(name, value);
        }
    }
    env.into_iter()
        .map(|(name, value)| {
            let mut entry = Zeroizing::new(Vec::with_capacity(name.len() + value.len() + 2));
            entry.extend_from_slice(name.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(&value);
            entry.push(0);
            entry
        })
        .collect()
}

fn check_bindings(bindings: &[WireBinding]) -> Result<()> {
    if bindings.is_empty() {
        bail!("hush run needs at least one --secret or --fd binding");
    }
    let fds = bindings
        .iter()
        .filter(|b| b.delivery == Delivery::Fd)
        .count();
    if fds > MAX_DELIVERY_FDS {
        bail!("at most {MAX_DELIVERY_FDS} --fd bindings per run");
    }
    let mut seen = HashSet::new();
    for binding in bindings {
        let var = child_var(&binding.env, binding.delivery);
        if is_forbidden_env(&var) {
            bail!("refusing to set {var}: BWS_, LD_, and DYLD_ names never reach a child");
        }
        if !seen.insert(var.clone()) {
            bail!("env var '{var}' is targeted by more than one binding");
        }
    }
    Ok(())
}

/// Same rule as `hush get`: one exact key match, optionally narrowed by project.
fn resolve_key<'a>(catalog: &'a [SecretMeta], binding: &WireBinding) -> Result<&'a SecretMeta> {
    let key = &binding.key;
    let matches: Vec<&SecretMeta> = catalog
        .iter()
        .filter(|meta| &meta.key == key)
        .filter(|meta| {
            binding
                .project
                .as_deref()
                .is_none_or(|q| meta.in_project(q))
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(anyhow!("no secret with key '{key}'")),
        _ => Err(anyhow!(
            "multiple secrets named '{key}'. Narrow with --project"
        )),
    }
}

/// The daemon drops privileges only as root. A same-user daemon may run a child
/// only for its own uid, and then keeps its identity unchanged.
pub(crate) fn identity_switch(identity: Identity) -> Result<Option<Identity>> {
    // SAFETY: geteuid takes no arguments and cannot fail.
    let euid = unsafe { libc::geteuid() };
    decide_switch(euid, identity)
}

pub(crate) fn decide_switch(euid: u32, identity: Identity) -> Result<Option<Identity>> {
    if euid == 0 {
        Ok(Some(identity))
    } else if euid == identity.uid {
        Ok(None)
    } else {
        bail!(
            "this daemon is not root and cannot run a child as uid {}",
            identity.uid
        )
    }
}
