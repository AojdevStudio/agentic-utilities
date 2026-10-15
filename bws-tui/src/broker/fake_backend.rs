//! Test vaults. Values live only inside these structs and leave exclusively
//! through [`VaultBackend::fetch`]. Counters let tests prove that a refused
//! request fetched or wrote nothing.

use super::backend::VaultBackend;
use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub(crate) struct FakeBackend {
    secrets: Mutex<HashMap<String, (SecretMeta, Vec<u8>)>>,
    fetches: AtomicUsize,
    writes: AtomicUsize,
    write_delay: Duration,
    catalog_delay: Duration,
    lose_delete_reply: bool,
}

impl FakeBackend {
    pub(crate) fn new(secrets: HashMap<String, (SecretMeta, String)>) -> Self {
        Self {
            secrets: Mutex::new(
                secrets
                    .into_iter()
                    .map(|(id, (meta, value))| (id, (meta, value.into_bytes())))
                    .collect(),
            ),
            fetches: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            write_delay: Duration::ZERO,
            catalog_delay: Duration::ZERO,
            lose_delete_reply: false,
        }
    }

    /// Deletes commit and then fail, like a provider reply lost after the change.
    pub(crate) fn with_lost_delete_reply(mut self) -> Self {
        self.lose_delete_reply = true;
        self
    }

    /// Every write waits this long first, which widens race windows in tests.
    pub(crate) fn with_write_delay(mut self, delay: Duration) -> Self {
        self.write_delay = delay;
        self
    }

    /// Every catalog read waits this long first, like a slow provider.
    pub(crate) fn with_catalog_delay(mut self, delay: Duration) -> Self {
        self.catalog_delay = delay;
        self
    }

    pub(crate) fn fetch_count(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }

    pub(crate) fn write_count(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    /// The stored value, for asserting what a write stored.
    pub(crate) fn stored(&self, id: &str) -> Option<Vec<u8>> {
        let secrets = self.secrets.lock().unwrap();
        secrets.get(id).map(|(_, value)| value.clone())
    }

    fn record_write(&self) {
        self.writes.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.write_delay);
    }

    fn project_name(&self, id: &str) -> Option<String> {
        self.projects()
            .ok()?
            .into_iter()
            .find(|project| project.id == id)
            .map(|project| project.name)
    }
}

impl VaultBackend for FakeBackend {
    fn catalog(&self) -> Result<Vec<SecretMeta>> {
        std::thread::sleep(self.catalog_delay);
        let secrets = self.secrets.lock().unwrap();
        let mut metas: Vec<SecretMeta> = secrets.values().map(|(meta, _)| meta.clone()).collect();
        metas.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(metas)
    }

    fn fetch(&self, secret: &SecretRef) -> Result<SecretMaterial> {
        let SecretRef::Id(id) = secret else {
            bail!("backends fetch only resolved secret ids");
        };
        self.fetches.fetch_add(1, Ordering::SeqCst);
        let secrets = self.secrets.lock().unwrap();
        let (_, value) = secrets.get(id).context("unknown secret id")?;
        Ok(SecretMaterial::new(value.clone()))
    }

    fn projects(&self) -> Result<Vec<ProjectMeta>> {
        let secrets = self.secrets.lock().unwrap();
        let mut projects: Vec<ProjectMeta> = secrets
            .values()
            .filter_map(|(meta, _)| {
                Some(ProjectMeta {
                    id: meta.project_id.clone()?,
                    name: meta.project.clone()?,
                })
            })
            .collect();
        projects.sort_by(|left, right| left.id.cmp(&right.id));
        projects.dedup();
        Ok(projects)
    }

    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        self.record_write();
        let project = self.project_name(&secret.project_id);
        let mut secrets = self.secrets.lock().unwrap();
        let id = format!("created-{}", secrets.len() + 1);
        let meta = SecretMeta {
            id: id.clone(),
            key: secret.key.clone(),
            project,
            project_id: Some(secret.project_id.clone()),
            note: secret.note.clone(),
        };
        secrets.insert(id, (meta.clone(), value.expose_bytes().to_vec()));
        Ok(meta)
    }

    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        self.record_write();
        let mut secrets = self.secrets.lock().unwrap();
        let (meta, stored) = secrets.get_mut(&secret.id).ok_or(WriteFailure::Rejected)?;
        meta.key = secret.key.clone();
        meta.note = secret.note.clone();
        *stored = value.expose_bytes().to_vec();
        Ok(meta.clone())
    }

    fn delete(&self, id: &str) -> Result<(), WriteFailure> {
        self.record_write();
        let mut secrets = self.secrets.lock().unwrap();
        secrets.remove(id).ok_or(WriteFailure::Rejected)?;
        if self.lose_delete_reply {
            return Err(WriteFailure::Uncertain);
        }
        Ok(())
    }
}

/// Every call fails. Tests use it to prove connected errors propagate.
pub(crate) struct FailingBackend;

impl VaultBackend for FailingBackend {
    fn catalog(&self) -> Result<Vec<SecretMeta>> {
        Err(anyhow!("backend exploded"))
    }

    fn fetch(&self, _secret: &SecretRef) -> Result<SecretMaterial> {
        Err(anyhow!("backend exploded"))
    }

    fn projects(&self) -> Result<Vec<ProjectMeta>> {
        Err(anyhow!("backend exploded"))
    }

    fn create(&self, _: &NewSecret, _: &SecretMaterial) -> Result<SecretMeta, WriteFailure> {
        Err(WriteFailure::Rejected)
    }

    fn update(&self, _: &SecretUpdate, _: &SecretMaterial) -> Result<SecretMeta, WriteFailure> {
        Err(WriteFailure::Rejected)
    }

    fn delete(&self, _id: &str) -> Result<(), WriteFailure> {
        Err(WriteFailure::Rejected)
    }
}
