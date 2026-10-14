//! Resolution, value input, and daemon writes. One mutation lock covers the
//! fresh duplicate lookup and the single write, so two daemon writers never
//! both create one key. The lock is never held while a human types. Writers
//! outside this daemon (the web vault, another machine) are not covered, and
//! an uncertain write is reported, never retried or rolled back.

use super::approve::APPROVAL_TIMEOUT;
use super::backend::VaultBackend;
use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use super::operation::{Outcome, Refusal, SecretSelector};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Mutex;
use std::time::Instant;
use zeroize::Zeroizing;

/// Largest value the daemon reads from a pipe or a prompt.
pub(crate) const MAX_VALUE: usize = 64 * 1024;

pub(crate) fn vault_failed(operation: &str) -> Refusal {
    Refusal::VaultFailed {
        operation: operation.into(),
    }
}

fn write_refusal(failure: WriteFailure, operation: &str) -> Refusal {
    match failure {
        WriteFailure::Rejected => vault_failed(operation),
        WriteFailure::Uncertain => Refusal::WriteUncertain {
            operation: operation.into(),
        },
    }
}

/// Exactly one secret, read fresh from the vault.
pub(crate) fn resolve(
    backend: &dyn VaultBackend,
    selector: &SecretSelector,
    operation: &str,
) -> Result<SecretMeta, Refusal> {
    let catalog = backend.catalog().map_err(|_| vault_failed(operation))?;
    let (key, matches): (&str, Vec<SecretMeta>) = match selector {
        SecretSelector::Id { id } => (id, catalog.into_iter().filter(|m| &m.id == id).collect()),
        SecretSelector::Key { key, project } => (
            key,
            catalog
                .into_iter()
                .filter(|m| &m.key == key)
                .filter(|m| project.as_deref().is_none_or(|q| m.in_project(q)))
                .collect(),
        ),
    };
    match <[SecretMeta; 1]>::try_from(matches) {
        Ok([one]) => Ok(one),
        Err(matches) if matches.is_empty() => Err(Refusal::NotFound { key: key.into() }),
        Err(matches) => Err(Refusal::Ambiguous {
            key: key.into(),
            count: matches.len() as u32,
        }),
    }
}

/// A project by exact id, else by unique case-insensitive name.
pub(crate) fn resolve_project(
    backend: &dyn VaultBackend,
    query: &str,
) -> Result<ProjectMeta, Refusal> {
    let projects = backend.projects().map_err(|_| vault_failed("add"))?;
    if let Some(project) = projects.iter().find(|p| p.id == query) {
        return Ok(project.clone());
    }
    let mut named = projects
        .into_iter()
        .filter(|p| p.name.eq_ignore_ascii_case(query));
    match (named.next(), named.next()) {
        (Some(project), None) => Ok(project),
        (None, _) => Err(Refusal::ProjectNotFound {
            query: query.into(),
        }),
        (Some(_), Some(_)) => Err(Refusal::ProjectAmbiguous {
            query: query.into(),
        }),
    }
}

/// Read a passed pipe or file to end of file, bounded in size and time.
pub(crate) fn read_input(fd: OwnedFd) -> Result<SecretMaterial, Refusal> {
    let deadline = Instant::now() + APPROVAL_TIMEOUT;
    let mut input = File::from(fd);
    let mut value = Zeroizing::new(Vec::with_capacity(MAX_VALUE + 1));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let mut poll = libc::pollfd {
            fd: input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = left.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: one valid pollfd.
        if left.is_zero() || unsafe { libc::poll(&mut poll, 1, millis) } == 0 {
            return Err(invalid("the value input did not finish in time"));
        }
        let read = input
            .read(&mut chunk[..])
            .map_err(|_| invalid("the value input could not be read"))?;
        if read == 0 {
            return material(value);
        }
        if value.len() + read > MAX_VALUE {
            return Err(invalid("the value is larger than 64 KiB"));
        }
        value.extend_from_slice(&chunk[..read]);
    }
}

/// Copy into an exact-size buffer so the boxed value never reallocates; the
/// read buffer zeroes itself on drop.
pub(crate) fn material(value: Zeroizing<Vec<u8>>) -> Result<SecretMaterial, Refusal> {
    if value.is_empty() {
        return Err(invalid("the value is empty"));
    }
    Ok(SecretMaterial::new(value.to_vec()))
}

pub(crate) fn invalid(reason: &str) -> Refusal {
    Refusal::InvalidValue {
        reason: reason.into(),
    }
}

/// Create once, unless the key already exists in the project. Never an overwrite.
pub(crate) fn add(
    backend: &dyn VaultBackend,
    lock: &Mutex<()>,
    project: &ProjectMeta,
    key: String,
    note: Option<String>,
    value: &SecretMaterial,
) -> Result<Outcome, Refusal> {
    let _guard = lock.lock().map_err(|_| vault_failed("add"))?;
    let catalog = backend.catalog().map_err(|_| vault_failed("add"))?;
    let same = |m: &&SecretMeta| m.key == key && m.project_id.as_deref() == Some(&project.id);
    if let Some(existing) = catalog.iter().find(same) {
        return Err(Refusal::AlreadyExists {
            key,
            id: existing.id.clone(),
        });
    }
    let new = NewSecret {
        project_id: project.id.clone(),
        key,
        note: note.unwrap_or_default(),
    };
    let mut meta = backend
        .create(&new, value)
        .map_err(|failure| write_refusal(failure, "add"))?;
    meta.project = Some(project.name.clone());
    Ok(Outcome::Added { meta })
}

/// Write the full record once. A kept value is fetched inside the daemon,
/// because the provider update replaces every field.
pub(crate) fn edit(
    backend: &dyn VaultBackend,
    lock: &Mutex<()>,
    target: &SecretMeta,
    new_key: Option<String>,
    note: Option<String>,
    value: Option<SecretMaterial>,
) -> Result<Outcome, Refusal> {
    let _guard = lock.lock().map_err(|_| vault_failed("edit"))?;
    let catalog = backend.catalog().map_err(|_| vault_failed("edit"))?;
    let Some(current) = catalog.iter().find(|m| m.id == target.id) else {
        return Err(Refusal::NotFound {
            key: target.key.clone(),
        });
    };
    let key = new_key.unwrap_or_else(|| current.key.clone());
    let clash = |m: &&SecretMeta| m.key == key && m.project_id == current.project_id;
    if let Some(other) = catalog.iter().filter(|m| m.id != current.id).find(clash) {
        return Err(Refusal::AlreadyExists {
            key,
            id: other.id.clone(),
        });
    }
    let value = match value {
        Some(value) => value,
        None => backend
            .fetch(&SecretRef::Id(current.id.clone()))
            .map_err(|_| vault_failed("edit"))?,
    };
    let update = SecretUpdate {
        id: current.id.clone(),
        project_id: current.project_id.clone(),
        key,
        note: note.unwrap_or_else(|| current.note.clone()),
    };
    let mut meta = backend
        .update(&update, &value)
        .map_err(|failure| write_refusal(failure, "edit"))?;
    meta.project = current.project.clone();
    Ok(Outcome::Edited { meta })
}
