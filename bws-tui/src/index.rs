//! Metadata-only index of the vault, cached on disk so `get`/`exec` can
//! resolve key → secret id locally and fetch exactly one secret from BWS.
//!
//! Invariant: `Entry` has no value field. This module is the only thing hush
//! ever writes to the cache, so secret values never touch disk.

use crate::bws::{Project, Secret};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entry {
    pub id: String,
    pub key: String,
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub note: String,
}

impl Entry {
    /// `--project` accepts a project name (case-insensitive) or UUID.
    pub fn in_project(&self, query: &str) -> bool {
        self.project
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(query))
            || self.project_id.as_deref() == Some(query)
    }
}

pub fn build(secrets: &[Secret], projects: &[Project]) -> Vec<Entry> {
    secrets
        .iter()
        .map(|secret| Entry {
            id: secret.id.clone(),
            key: secret.key.clone(),
            project: secret.project_id.as_deref().and_then(|id| {
                projects
                    .iter()
                    .find(|project| project.id == id)
                    .map(|project| project.name.clone())
            }),
            project_id: secret.project_id.clone(),
            note: secret.note.clone(),
        })
        .collect()
}

fn cache_path() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CACHE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".cache"),
    };
    Ok(base.join("hush").join("index.json"))
}

/// A missing or unreadable index is not an error: callers fall back to a live
/// sync, which rewrites it (the documented cache-rebuild recovery path).
pub fn load() -> Option<Vec<Entry>> {
    let raw = std::fs::read_to_string(cache_path().ok()?).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save(entries: &[Entry]) -> Result<()> {
    let path = cache_path()?;
    let dir = path.parent().context("cache path has no parent")?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, serde_json::to_string_pretty(entries)?)
        .with_context(|| format!("failed to write index at {}", path.display()))?;
    Ok(())
}

/// Find exactly one entry by key, optionally scoped by project name (or id).
pub fn resolve<'a>(entries: &'a [Entry], key: &str, project: Option<&str>) -> Result<&'a Entry> {
    let matches: Vec<&Entry> = entries
        .iter()
        .filter(|entry| entry.key == key)
        .filter(|entry| project.is_none_or(|query| entry.in_project(query)))
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(anyhow!("no secret with key '{key}'")),
        _ => Err(anyhow!(
            "multiple secrets named '{key}' — narrow with --project"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(id: &str, key: &str, project_id: Option<&str>) -> Secret {
        Secret {
            id: id.into(),
            key: key.into(),
            value: "TOPSECRET-VALUE".into(),
            note: "scope: test".into(),
            project_id: project_id.map(Into::into),
        }
    }

    #[test]
    fn serialized_index_never_contains_secret_values() {
        let projects = vec![Project {
            id: "p1".into(),
            name: "API Keys".into(),
        }];
        let entries = build(&[secret("s1", "TOKEN", Some("p1"))], &projects);
        let json = serde_json::to_string(&entries).unwrap();
        assert!(!json.contains("TOPSECRET-VALUE"));
        assert!(!json.contains("value"));
        assert!(json.contains("API Keys"));
    }

    #[test]
    fn resolve_requires_an_unambiguous_match() {
        let projects = vec![
            Project {
                id: "p1".into(),
                name: "API Keys".into(),
            },
            Project {
                id: "p2".into(),
                name: "Logins".into(),
            },
        ];
        let entries = build(
            &[
                secret("s1", "TOKEN", Some("p1")),
                secret("s2", "TOKEN", Some("p2")),
            ],
            &projects,
        );
        assert!(resolve(&entries, "TOKEN", None).is_err());
        assert!(resolve(&entries, "MISSING", None).is_err());
        assert_eq!(resolve(&entries, "TOKEN", Some("logins")).unwrap().id, "s2");
        // Project UUIDs work wherever names do.
        assert_eq!(resolve(&entries, "TOKEN", Some("p1")).unwrap().id, "s1");
    }
}
