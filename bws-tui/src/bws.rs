use anyhow::{anyhow, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    pub id: String,
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub note: String,
    pub project_id: Option<String>,
}

fn run(args: &[&str]) -> Result<String> {
    let out = Command::new("bws")
        .args(args)
        .args(["-o", "json"])
        .output()
        .context("failed to run `bws` — is it installed and on PATH?")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "bws {} failed: {}",
            args.join(" "),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8(out.stdout)?)
}

pub fn list_projects() -> Result<Vec<Project>> {
    Ok(serde_json::from_str(&run(&["project", "list"])?)?)
}

/// Values are included in list output (bws 2.1.0), so no per-secret get is needed.
pub fn list_secrets(project_id: Option<&str>) -> Result<Vec<Secret>> {
    let mut args = vec!["secret", "list"];
    if let Some(p) = project_id {
        args.push(p);
    }
    Ok(serde_json::from_str(&run(&args)?)?)
}

pub fn create_secret(
    key: &str,
    value: &SecretString,
    project_id: &str,
    note: Option<&str>,
) -> Result<Secret> {
    // ponytail: bws 2.1.0 accepts values only as argv — briefly visible in `ps`.
    // No stdin/file input exists upstream; nothing a wrapper can do.
    let mut args = vec!["secret", "create", key, value.expose_secret(), project_id];
    if let Some(n) = note {
        args.extend(["--note", n]);
    }
    Ok(serde_json::from_str(&run(&args)?)?)
}

pub fn edit_secret(
    id: &str,
    key: Option<&str>,
    value: Option<&SecretString>,
    note: Option<&str>,
) -> Result<Secret> {
    let mut args = vec!["secret", "edit"];
    if let Some(k) = key {
        args.extend(["--key", k]);
    }
    if let Some(v) = value {
        args.extend(["--value", v.expose_secret()]);
    }
    if let Some(n) = note {
        args.extend(["--note", n]);
    }
    args.push(id);
    Ok(serde_json::from_str(&run(&args)?)?)
}

pub fn delete_secret(id: &str) -> Result<()> {
    run(&["secret", "delete", id])?;
    Ok(())
}

/// Match a `--project` query against the live list: exact id, then case-insensitive name.
pub fn resolve_project<'a>(projects: &'a [Project], query: &str) -> Result<&'a Project> {
    if let Some(p) = projects.iter().find(|p| p.id == query) {
        return Ok(p);
    }
    let matches: Vec<&Project> = projects
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(query))
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(anyhow!("no project named '{query}'")),
        _ => Err(anyhow!("ambiguous project name '{query}' — use the UUID")),
    }
}

/// Find exactly one secret by key (optionally scoped to a project).
pub fn find_by_key<'a>(
    secrets: &'a [Secret],
    key: &str,
    project_id: Option<&str>,
) -> Result<&'a Secret> {
    let matches: Vec<&Secret> = secrets
        .iter()
        .filter(|s| s.key == key)
        .filter(|s| project_id.is_none_or(|p| s.project_id.as_deref() == Some(p)))
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(anyhow!("no secret with key '{key}'")),
        _ => Err(anyhow!(
            "multiple secrets named '{key}' — narrow with --project"
        )),
    }
}
