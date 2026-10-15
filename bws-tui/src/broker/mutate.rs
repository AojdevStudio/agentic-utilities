//! Resolution and daemon writes. One mutation lock covers the fresh lookup and
//! the single write, so two daemon writers never both create one key and a
//! delete never removes a secret renamed or moved after approval. The lock is
//! never held while a human types, and a client that leaves while waiting for
//! it stops waiting. Writers outside this daemon (the web vault, another
//! machine) can still change a secret between that lookup and the write,
//! because the provider delete takes only ids. An uncertain write is
//! reported, never retried or rolled back.

use super::backend::VaultBackend;
use super::fields::{check_key, check_note, check_value};
use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use super::operation::{Outcome, Refusal, SecretSelector};
use super::request::{RequestControl, WriteAttempt};
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::Duration;

/// How often a waiting writer retries the lock. The client socket wakes the
/// wait at once if the client leaves.
const LOCK_RETRY: Duration = Duration::from_millis(5);

pub(crate) fn vault_failed(operation: &str) -> Refusal {
    Refusal::VaultFailed {
        operation: operation.into(),
    }
}

pub(crate) fn write_refusal(failure: WriteFailure, operation: &str) -> Refusal {
    match failure {
        WriteFailure::Rejected => vault_failed(operation),
        WriteFailure::Stopped => Refusal::ApprovalCancelled,
        WriteFailure::Uncertain => Refusal::WriteUncertain {
            operation: operation.into(),
        },
    }
}

/// A stopped request reports the stop. Anything else is a vault failure.
pub(crate) fn read_failed(control: &RequestControl<'_>, operation: &str) -> Refusal {
    match control.check() {
        Err(_) => Refusal::ApprovalCancelled,
        Ok(()) => vault_failed(operation),
    }
}

/// The request is still wanted.
pub(crate) fn live(control: &RequestControl<'_>) -> Result<(), Refusal> {
    control.check().map_err(|_| Refusal::ApprovalCancelled)
}

/// Exactly one secret, read fresh from the vault.
pub(crate) fn resolve(
    backend: &dyn VaultBackend,
    selector: &SecretSelector,
    operation: &str,
    control: &RequestControl<'_>,
) -> Result<SecretMeta, Refusal> {
    let catalog = backend
        .catalog(control)
        .map_err(|_| read_failed(control, operation))?;
    live(control)?;
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
    control: &RequestControl<'_>,
) -> Result<ProjectMeta, Refusal> {
    let projects = backend
        .projects(control)
        .map_err(|_| read_failed(control, "add"))?;
    live(control)?;
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

/// Take the mutation lock without ever blocking past the client or the
/// deadline. A plain `lock` would keep waiting after the client left.
fn admit<'m>(
    lock: &'m Mutex<()>,
    control: &RequestControl<'_>,
    operation: &str,
) -> Result<MutexGuard<'m, ()>, Refusal> {
    loop {
        live(control)?;
        match lock.try_lock() {
            Ok(guard) => return live(control).map(|()| guard),
            Err(TryLockError::WouldBlock) => control
                .nap(LOCK_RETRY)
                .map_err(|_| Refusal::ApprovalCancelled)?,
            Err(TryLockError::Poisoned(_)) => return Err(vault_failed(operation)),
        }
    }
}

/// Create once, unless the key already exists in the project. Never an overwrite.
pub(crate) fn add(
    backend: &dyn VaultBackend,
    lock: &Mutex<()>,
    control: &RequestControl<'_>,
    project: &ProjectMeta,
    key: String,
    note: Option<String>,
    value: &SecretMaterial,
) -> Result<Outcome, Refusal> {
    check_key(&key)?;
    let _guard = admit(lock, control, "add")?;
    let catalog = backend
        .catalog(control)
        .map_err(|_| read_failed(control, "add"))?;
    live(control)?;
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
    let mut attempt = WriteAttempt::new(control);
    let mut meta = backend
        .create(&new, value, &mut attempt)
        .map_err(|failure| write_refusal(failure, "add"))?;
    meta.project = Some(project.name.clone());
    Ok(Outcome::Added { meta })
}

/// Write the full record once. A kept value is fetched inside the daemon,
/// because the provider update replaces every field. Kept fields pass the
/// same provider rules as new ones before the write starts.
pub(crate) fn edit(
    backend: &dyn VaultBackend,
    lock: &Mutex<()>,
    control: &RequestControl<'_>,
    target: &SecretMeta,
    new_key: Option<String>,
    note: Option<String>,
    value: Option<SecretMaterial>,
) -> Result<Outcome, Refusal> {
    let _guard = admit(lock, control, "edit")?;
    let catalog = backend
        .catalog(control)
        .map_err(|_| read_failed(control, "edit"))?;
    live(control)?;
    let Some(current) = catalog.iter().find(|m| m.id == target.id) else {
        return Err(Refusal::NotFound {
            key: target.key.clone(),
        });
    };
    let key = new_key.unwrap_or_else(|| current.key.clone());
    check_key(&key)?;
    let note = note.unwrap_or_else(|| current.note.clone());
    check_note(&note)?;
    let clash = |m: &&SecretMeta| m.key == key && m.project_id == current.project_id;
    if let Some(other) = catalog.iter().filter(|m| m.id != current.id).find(clash) {
        return Err(Refusal::AlreadyExists {
            key,
            id: other.id.clone(),
        });
    }
    let value = match value {
        Some(value) => value,
        None => {
            let kept = backend
                .fetch(&SecretRef::Id(current.id.clone()), control)
                .map_err(|_| read_failed(control, "edit"))?;
            check_value(kept.expose_bytes())?;
            kept
        }
    };
    let update = SecretUpdate {
        id: current.id.clone(),
        project_id: current.project_id.clone(),
        key,
        note,
    };
    let mut attempt = WriteAttempt::new(control);
    let mut meta = backend
        .update(&update, &value, &mut attempt)
        .map_err(|failure| write_refusal(failure, "edit"))?;
    meta.project = current.project.clone();
    Ok(Outcome::Edited { meta })
}

/// Delete the approved secret once, only while the vault still shows the id,
/// key, and project the human approved. Anything else is refused as missing.
pub(crate) fn delete(
    backend: &dyn VaultBackend,
    lock: &Mutex<()>,
    control: &RequestControl<'_>,
    approved: &SecretMeta,
) -> Result<Outcome, Refusal> {
    let _guard = admit(lock, control, "delete")?;
    let catalog = backend
        .catalog(control)
        .map_err(|_| read_failed(control, "delete"))?;
    live(control)?;
    let same = |m: &SecretMeta| {
        m.id == approved.id
            && m.key == approved.key
            && m.project_id == approved.project_id
            && m.project == approved.project
    };
    if !catalog.iter().any(same) {
        return Err(Refusal::NotFound {
            key: approved.key.clone(),
        });
    }
    let mut attempt = WriteAttempt::new(control);
    backend
        .delete(&approved.id, &mut attempt)
        .map_err(|failure| write_refusal(failure, "delete"))?;
    Ok(Outcome::Deleted {
        id: approved.id.clone(),
        key: approved.key.clone(),
    })
}
