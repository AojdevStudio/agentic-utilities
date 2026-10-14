//! Vault access. The `bws` wire structs stay private to this module.

use super::material::SecretMaterial;
use super::meta::{NewSecret, ProjectMeta, SecretMeta, SecretRef, SecretUpdate, WriteFailure};
use super::sdk::SdkWriter;
use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

/// Hard cap on bws stdout. `bws secret list` returns every value at once.
pub(crate) const MAX_BWS_OUTPUT: usize = 16 * 1024 * 1024;

/// Hard deadline for one bws call.
pub(crate) const BWS_TIMEOUT: Duration = Duration::from_secs(60);

/// The daemon's only path to stored values. `catalog` never returns values.
/// Writes take values as [`SecretMaterial`] and never put them in argv.
pub(crate) trait VaultBackend: Send + Sync {
    fn catalog(&self) -> Result<Vec<SecretMeta>>;
    fn fetch(&self, secret: &SecretRef) -> Result<SecretMaterial>;
    fn projects(&self) -> Result<Vec<ProjectMeta>>;
    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure>;
    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure>;
    fn delete(&self, id: &str) -> Result<()>;
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

/// Real backend. Reads and id-only deletes shell out to `bws` by absolute path
/// with a cleared environment. `bws` takes write values in argv, so value
/// writes go through the Bitwarden SDK instead.
pub(crate) struct BwsBackend {
    bws_path: PathBuf,
    token: SecretString,
    writer: SdkWriter,
}

impl BwsBackend {
    pub(crate) fn new(bws_path: PathBuf, token: SecretString) -> Self {
        let writer = SdkWriter::cloud(token.clone());
        Self {
            bws_path,
            token,
            writer,
        }
    }

    fn run_bws(&self, args: &[&str]) -> Result<Zeroizing<String>> {
        self.run_bws_capped(args, MAX_BWS_OUTPUT)
    }

    /// Run bws with a byte cap on stdout. Stderr is discarded.
    pub(crate) fn run_bws_capped(&self, args: &[&str], cap: usize) -> Result<Zeroizing<String>> {
        self.run_bws_with_deadline(args, cap, BWS_TIMEOUT)
    }

    /// Run bws with a byte cap on stdout and a hard deadline. Stderr is discarded.
    /// The output holds secret values, so the returned string zeroizes on drop.
    pub(crate) fn run_bws_with_deadline(
        &self,
        args: &[&str],
        cap: usize,
        timeout: Duration,
    ) -> Result<Zeroizing<String>> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("failed to run bws")?;
        let child = Command::new(&self.bws_path)
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("BWS_ACCESS_TOKEN", self.token.expose_secret())
            .args(args)
            .args(["-o", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to run bws")?;
        let mut guard = ChildGuard::new(child);
        let stdout = guard.take_stdout().context("failed to run bws")?;
        let (mut bytes, overflow) = read_capped_until(stdout, cap, deadline)?;
        if overflow {
            return Err(anyhow!("bws output exceeded the size limit"));
        }
        let status = guard.wait_until(deadline)?;
        if !status.success() {
            let name = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
            return Err(anyhow!("bws {name} failed ({status})"));
        }
        let text = match String::from_utf8(std::mem::take(&mut bytes)) {
            Ok(text) => text,
            Err(error) => {
                let mut raw = error.into_bytes();
                raw.zeroize();
                bail!("bws produced invalid UTF-8");
            }
        };
        Ok(Zeroizing::new(text))
    }
}

/// Read at most `cap` bytes. The flag is true when more data remains.
fn read_capped(reader: &mut impl Read, cap: usize) -> Result<(Vec<u8>, bool)> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            return Ok((out, false));
        }
        if out.len() + read > cap {
            let keep = cap.saturating_sub(out.len());
            out.extend_from_slice(&chunk[..keep]);
            return Ok((out, true));
        }
        out.extend_from_slice(&chunk[..read]);
    }
}

/// Read at most `cap` bytes before the deadline. Stdout waits on a helper thread.
fn read_capped_until(
    mut stdout: std::process::ChildStdout,
    cap: usize,
    deadline: Instant,
) -> Result<(Vec<u8>, bool)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let _reader = std::thread::spawn(move || {
        let _ = sender.send(read_capped(&mut stdout, cap));
    });
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(result) => result.context("failed to run bws"),
        Err(_) => Err(anyhow!("bws timed out")),
    }
}

/// Kills and reaps the child on drop unless it already exited through `wait_until`.
struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    /// Wrap a spawned child.
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    /// Take the piped stdout handle.
    fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.as_mut()?.stdout.take()
    }

    /// Wait for exit before the deadline.
    fn wait_until(&mut self, deadline: Instant) -> Result<std::process::ExitStatus> {
        loop {
            let child = match self.child.as_mut() {
                Some(child) => child,
                None => return Err(anyhow!("failed to run bws")),
            };
            if let Some(status) = child.try_wait().context("failed to run bws")? {
                self.child = None;
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(anyhow!("bws timed out"));
            }
            std::thread::sleep(
                Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
        let SecretRef::Id(id) = secret else {
            bail!("backends fetch only resolved secret ids");
        };
        let wire: BwsSecret = serde_json::from_str(&self.run_bws(&["secret", "get", id])?)?;
        let bytes = wire.value.expose_secret().as_bytes().to_vec();
        Ok(SecretMaterial::new(bytes))
    }

    fn projects(&self) -> Result<Vec<ProjectMeta>> {
        let projects: Vec<BwsProject> = serde_json::from_str(&self.run_bws(&["project", "list"])?)?;
        Ok(projects.into_iter().map(ProjectMeta::from).collect())
    }

    fn create(
        &self,
        secret: &NewSecret,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.create(secret, value)
    }

    fn update(
        &self,
        secret: &SecretUpdate,
        value: &SecretMaterial,
    ) -> Result<SecretMeta, WriteFailure> {
        self.writer.update(secret, value)
    }

    fn delete(&self, id: &str) -> Result<()> {
        self.run_bws(&["secret", "delete", id])?;
        Ok(())
    }
}
