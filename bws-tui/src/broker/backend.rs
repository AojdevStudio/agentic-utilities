//! Vault access. The `bws` wire structs stay private to this module.

use super::bws_child::{run, BwsCall, BwsError};
use super::children::Children;
use super::daemon::MAX_CONNECTIONS;
use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use super::request::{RequestControl, WriteAttempt};
use super::sdk::SdkWriter;
use anyhow::{bail, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use zeroize::Zeroizing;

/// Hard cap on bws stdout. `bws secret list` returns every value at once.
pub(crate) const MAX_BWS_OUTPUT: usize = 16 * 1024 * 1024;

/// Hard deadline for one bws call.
pub(crate) const BWS_TIMEOUT: Duration = Duration::from_secs(60);

/// The daemon's only path to stored values. `catalog` never returns values.
/// Writes take values as [`SecretMaterial`] and never put them in argv.
/// Every wait ends at the earlier of its own limit and the caller's request
/// deadline, and stops when a human request's client leaves. A detached
/// control keeps each call's own limit.
pub(crate) trait VaultBackend: Send + Sync {
    fn catalog(&self, control: &RequestControl<'_>) -> Result<Vec<SecretMeta>>;
    fn fetch(&self, secret: &SecretRef, control: &RequestControl<'_>) -> Result<SecretMaterial>;
    fn projects(&self, control: &RequestControl<'_>) -> Result<Vec<ProjectMeta>>;
    /// Calls [`WriteAttempt::begin_remote`] immediately before the first step
    /// that can reach the provider.
    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure>;
    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure>;
    /// A failure after the request may have reached the vault is `Uncertain`.
    fn delete(&self, id: &str, attempt: &mut WriteAttempt<'_>) -> Result<(), WriteFailure>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BwsProject {
    id: String,
    name: String,
}

impl From<BwsProject> for ProjectMeta {
    fn from(project: BwsProject) -> Self {
        Self {
            id: project.id,
            name: project.name,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BwsSecret {
    id: String,
    key: String,
    /// Values zeroize on drop even inside the wire struct.
    value: SecretString,
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

/// Real backend. Reads and id-only deletes run `bws` by absolute path with a
/// cleared environment. `bws` takes write values in argv, so value writes go
/// through the Bitwarden SDK instead.
pub(crate) struct BwsBackend {
    bws_path: PathBuf,
    token: SecretString,
    writer: SdkWriter,
    children: Children,
}

impl BwsBackend {
    pub(crate) fn new(bws_path: PathBuf, token: SecretString) -> Self {
        let writer = SdkWriter::cloud(token.clone());
        Self {
            bws_path,
            token,
            writer,
            children: Children::new(MAX_CONNECTIONS),
        }
    }

    fn call<'a>(&'a self, args: &'a [&'a str]) -> BwsCall<'a> {
        BwsCall {
            path: &self.bws_path,
            token: &self.token,
            args,
        }
    }

    fn run_bws(&self, args: &[&str], control: &RequestControl<'_>) -> Result<Zeroizing<String>> {
        Ok(run(
            &self.call(args),
            MAX_BWS_OUTPUT,
            BWS_TIMEOUT,
            control,
            &self.children,
        )?)
    }

    /// Run bws with a byte cap on stdout and a hard deadline, outside any
    /// request. Stderr is discarded.
    #[cfg(test)]
    pub(crate) fn run_bws_with_deadline(
        &self,
        args: &[&str],
        cap: usize,
        timeout: Duration,
    ) -> Result<Zeroizing<String>> {
        let control = RequestControl::detached();
        Ok(run(
            &self.call(args),
            cap,
            timeout,
            &control,
            &self.children,
        )?)
    }

    /// Child slots not yet released by a reap.
    #[cfg(test)]
    pub(crate) fn busy_children(&self) -> usize {
        self.children.busy()
    }
}

impl VaultBackend for BwsBackend {
    fn catalog(&self, control: &RequestControl<'_>) -> Result<Vec<SecretMeta>> {
        let projects: Vec<BwsProject> =
            serde_json::from_str(&self.run_bws(&["project", "list"], control)?)?;
        let secrets: Vec<BwsSecret> =
            serde_json::from_str(&self.run_bws(&["secret", "list"], control)?)?;
        Ok(secrets
            .into_iter()
            .map(|secret| drop_value(secret, &projects))
            .collect())
    }

    fn fetch(&self, secret: &SecretRef, control: &RequestControl<'_>) -> Result<SecretMaterial> {
        let SecretRef::Id(id) = secret else {
            bail!("backends fetch only resolved secret ids");
        };
        let wire: BwsSecret =
            serde_json::from_str(&self.run_bws(&["secret", "get", id], control)?)?;
        let bytes = wire.value.expose_secret().as_bytes().to_vec();
        Ok(SecretMaterial::new(bytes))
    }

    fn projects(&self, control: &RequestControl<'_>) -> Result<Vec<ProjectMeta>> {
        let projects: Vec<BwsProject> =
            serde_json::from_str(&self.run_bws(&["project", "list"], control)?)?;
        Ok(projects.into_iter().map(ProjectMeta::from).collect())
    }

    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.create(secret, value, attempt)
    }

    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
        attempt: &mut WriteAttempt<'_>,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.update(secret, value, attempt)
    }

    /// Only a delete proven never to have started is `Rejected`.
    fn delete(&self, id: &str, attempt: &mut WriteAttempt<'_>) -> Result<(), WriteFailure> {
        let args = ["secret", "delete", id];
        attempt.begin_remote()?;
        match run(
            &self.call(&args),
            MAX_BWS_OUTPUT,
            BWS_TIMEOUT,
            attempt.control(),
            &self.children,
        ) {
            Ok(_) => Ok(()),
            Err(BwsError::NotStarted(_)) => Err(WriteFailure::Rejected),
            Err(BwsError::Failed(_)) => Err(WriteFailure::Uncertain),
        }
    }
}
