//! A test double for the native boundary only. Account and manager state
//! live here. Every file, including the host key it creates, stays real.

use super::apply::TokenSource;
use super::host::{Host, Operation};
use super::layout::host_key_path;
use super::model::{Fact, Manager, NativeAccount};
use super::observe::Binaries;
use super::tree::Tree;
use anyhow::{bail, Result};
use secrecy::{ExposeSecret, SecretString};
use std::fs::File;
use std::io::Write;

pub(crate) const TOKEN: &str = "synthetic-install-token-5c1d";

pub(crate) struct FakeHost {
    pub anchor: Tree,
    pub systemd: Fact<u32>,
    pub manager: Manager,
    pub manager_fact: Option<Fact<Manager>>,
    pub account: Option<NativeAccount>,
    pub account_fact: Option<Fact<Option<NativeAccount>>>,
    pub log: Vec<String>,
    /// Fail the named operation, or `encrypt`, without doing it.
    pub fail: Option<&'static str>,
    /// After the named operation, the manager reports this fact.
    pub then_manager: Option<(&'static str, Fact<Manager>)>,
    pub pid: u32,
}

impl FakeHost {
    pub(crate) fn new(tag: &str) -> Self {
        Self {
            anchor: system_tree(tag),
            systemd: Fact::Known(255),
            manager: Manager { loaded: false, enabled: cfg!(target_os = "macos"), pid: None, stale: false },
            manager_fact: None,
            account: None,
            account_fact: None,
            log: Vec::new(),
            fail: None,
            then_manager: None,
            pid: 4242,
        }
    }

    pub(crate) fn binaries(&self) -> Binaries {
        Binaries { hush: self.anchor.path("/usr/local/bin/hush"), bws: None }
    }
}

/// A root-like system tree: directories, executables, and PAM includes.
pub(crate) fn system_tree(tag: &str) -> Tree {
    let tree = Tree::new(tag);
    for exe in ["/usr/local/bin/hush", "/usr/local/bin/bws", "/usr/sbin/nologin", "/usr/bin/false"] {
        tree.file(exe, b"#!/bin/false\n", 0o755);
    }
    for dir in ["/etc/systemd/system", "/var/log", "/var/lib/systemd", "/var/db", "/Library/LaunchDaemons", "/run", "/var/run"] {
        tree.dirs(dir);
    }
    tree.file("/etc/pam.d/common-auth", b"auth required pam_unix.so\n", 0o644);
    tree.file("/etc/pam.d/common-account", b"account required pam_unix.so\n", 0o644);
    tree
}

impl Host for FakeHost {
    fn systemd_version(&mut self) -> Fact<u32> {
        self.systemd.clone()
    }

    fn manager(&mut self) -> Fact<Manager> {
        self.manager_fact.clone().unwrap_or(Fact::Known(self.manager))
    }

    fn account(&mut self) -> Fact<Option<NativeAccount>> {
        self.account_fact.clone().unwrap_or(Fact::Known(self.account.clone()))
    }

    fn nobody_gid(&mut self) -> Fact<u32> {
        Fact::Known(65534)
    }

    fn used_uids(&mut self) -> Fact<Vec<u32>> {
        Fact::Known(vec![0, 1, 200, 201])
    }

    fn perform(&mut self, operation: &Operation) -> Result<()> {
        let name = operation.to_string();
        if self.fail == Some(name.as_str()) {
            bail!("native failure injected");
        }
        self.log.push(name.clone());
        match operation {
            Operation::CreateAccount(request) => {
                self.account = Some(NativeAccount {
                    uid: request.uid.unwrap_or(990),
                    gid: request.gid,
                    home: request.home.clone(),
                    shell: request.shell.clone(),
                    login_disabled: true,
                    unique: true,
                });
            }
            Operation::CreateHostKey => {
                self.anchor.file(host_key_path(), b"synthetic host key", 0o400);
            }
            Operation::Register => {
                self.manager.loaded = true;
                self.manager.stale = false;
                if cfg!(target_os = "macos") {
                    self.manager.pid = Some(self.pid);
                }
            }
            Operation::Enable => self.manager.enabled = true,
            Operation::Start => self.manager.pid = Some(self.pid),
        }
        if let Some((after, fact)) = &self.then_manager {
            if *after == name {
                self.manager_fact = Some(fact.clone());
            }
        }
        Ok(())
    }

    fn encrypt(&mut self, token: &SecretString, out: &mut File) -> Result<()> {
        if self.fail == Some("encrypt") {
            bail!("native failure injected");
        }
        self.log.push("encrypt".into());
        out.write_all(format!("ciphertext of {} bytes", token.expose_secret().len()).as_bytes())?;
        Ok(())
    }
}

/// Counts reads. `None` fails any read, proving no input was needed.
pub(crate) struct Input {
    pub token: Option<&'static str>,
    pub reads: usize,
}

impl TokenSource for Input {
    fn read_token(&mut self) -> Result<SecretString> {
        self.reads += 1;
        match self.token {
            Some(token) => Ok(SecretString::from(token.to_string())),
            None => bail!("no token input was expected"),
        }
    }
}
