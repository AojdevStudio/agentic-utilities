//! Index-backed retrieval: `sync`, scoped `get`, and discovery `list`.
//! Reading one secret means one secret crosses the wire (`bws secret get`);
//! discovery is always live truth and refreshes the index as a side effect.

use crate::{audit, bws, index};
use anyhow::Result;

/// Live-fetch projects and secrets, rebuild and persist the metadata index.
pub fn sync_index() -> Result<Vec<index::Entry>> {
    let projects = bws::list_projects()?;
    let secrets = bws::list_secrets(None)?;
    let entries = index::build(&secrets, &projects);
    index::save(&entries)?;
    Ok(entries)
}

/// Resolve key → entry from the cached index, falling back to a live sync
/// when the index is missing, corrupt, or doesn't know the key.
fn resolve(key: &str, project: Option<&str>) -> Result<index::Entry> {
    if let Some(entries) = index::load() {
        if let Ok(entry) = index::resolve(&entries, key, project) {
            return Ok(entry.clone());
        }
    }
    let entries = sync_index()?;
    index::resolve(&entries, key, project).cloned()
}

/// One secret over the wire. A stale cached id (secret rotated or deleted
/// since the last sync) gets one resync-and-retry; the retry's error is real.
pub fn fetch(key: &str, project: Option<&str>) -> Result<bws::Secret> {
    let entry = resolve(key, project)?;
    match bws::get_secret(&entry.id) {
        Ok(secret) => Ok(secret),
        Err(_) => {
            let entries = sync_index()?;
            let entry = index::resolve(&entries, key, project)?;
            bws::get_secret(&entry.id)
        }
    }
}

/// The audit line records the attempted read before the fetch, so the log
/// overcounts exposure rather than undercounting it.
pub fn get(key: &str, project: Option<&str>) -> Result<()> {
    audit::record("get", &[key])?;
    let secret = fetch(key, project)?;
    println!("{}", secret.value);
    Ok(())
}

pub fn list(project: Option<&str>, json: bool) -> Result<()> {
    let entries = sync_index()?;
    let entries: Vec<index::Entry> = entries
        .into_iter()
        .filter(|entry| project.is_none_or(|query| entry.in_project(query)))
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for entry in &entries {
            println!(
                "{}\t{}\t{}",
                entry.id,
                entry.key,
                entry.project.as_deref().unwrap_or("no project")
            );
        }
    }
    Ok(())
}

pub fn sync() -> Result<()> {
    audit::record("sync", &[])?;
    let entries = sync_index()?;
    println!("indexed {} secrets (metadata only)", entries.len());
    Ok(())
}
