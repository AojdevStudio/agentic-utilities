//! Vault access. The `bws` wire structs stay private to this module.

use super::material::SecretMaterial;
use super::meta::{SecretMeta, SecretRef};
use anyhow::{anyhow, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
#[cfg(test)]
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// The daemon's only path to stored values. `catalog` never returns values.
#[allow(dead_code)]
pub(crate) trait VaultBackend: Send + Sync {
    fn catalog(&self) -> Result<Vec<SecretMeta>>;
    fn fetch(&self, secret: &SecretRef) -> Result<SecretMaterial>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BwsProject {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BwsSecret {
    id: String,
    key: String,
    value: String,
    #[serde(default)]
    note: String,
    project_id: Option<String>,
}

fn drop_value(secret: BwsSecret, projects: &[BwsProject]) -> SecretMeta {
    let BwsSecret {
        id,
        key,
        note,
        project_id,
        value,
    } = secret;
    drop(value);
    SecretMeta {
        id,
        key,
        project: project_id.as_ref().and_then(|wanted| {
            projects
                .iter()
                .find(|p| &p.id == wanted)
                .map(|p| p.name.clone())
        }),
        project_id,
        note,
    }
}

/// Real backend. Shells out to `bws` by absolute path with a cleared environment.
pub(crate) struct BwsBackend {
    bws_path: PathBuf,
    token: SecretString,
}

impl BwsBackend {
    pub(crate) fn new(bws_path: PathBuf, token: SecretString) -> Self {
        Self { bws_path, token }
    }

    fn run_bws(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.bws_path)
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("BWS_ACCESS_TOKEN", self.token.expose_secret())
            .args(args)
            .args(["-o", "json"])
            .output()
            .context("failed to run bws")?;
        if !output.status.success() {
            let name = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
            return Err(anyhow!("bws {name} failed ({})", output.status));
        }
        Ok(String::from_utf8(output.stdout)?)
    }
}

impl VaultBackend for BwsBackend {
    fn catalog(&self) -> Result<Vec<SecretMeta>> {
        let projects: Vec<BwsProject> = serde_json::from_str(&self.run_bws(&["project", "list"])?)?;
        let secrets: Vec<BwsSecret> = serde_json::from_str(&self.run_bws(&["secret", "list"])?)?;
        Ok(secrets
            .into_iter()
            .map(|secret| drop_value(secret, &projects))
            .collect())
    }

    fn fetch(&self, secret: &SecretRef) -> Result<SecretMaterial> {
        let SecretRef::Id(id) = secret;
        let wire: BwsSecret = serde_json::from_str(&self.run_bws(&["secret", "get", id])?)?;
        Ok(SecretMaterial::new(wire.value.into_bytes()))
    }
}

/// In-memory backend for tests. Values live only in this struct and leave
/// exclusively through [`VaultBackend::fetch`].
#[cfg(test)]
pub(crate) struct FakeBackend {
    secrets: HashMap<String, (SecretMeta, String)>,
}

#[cfg(test)]
impl FakeBackend {
    pub(crate) fn new(secrets: HashMap<String, (SecretMeta, String)>) -> Self {
        Self { secrets }
    }
}

#[cfg(test)]
impl VaultBackend for FakeBackend {
    fn catalog(&self) -> Result<Vec<SecretMeta>> {
        let mut metas: Vec<SecretMeta> = self
            .secrets
            .values()
            .map(|(meta, _)| meta.clone())
            .collect();
        metas.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(metas)
    }

    fn fetch(&self, secret: &SecretRef) -> Result<SecretMaterial> {
        let SecretRef::Id(id) = secret;
        let (_, value) = self.secrets.get(id).context("unknown secret id")?;
        Ok(SecretMaterial::new(value.as_bytes().to_vec()))
    }
}
