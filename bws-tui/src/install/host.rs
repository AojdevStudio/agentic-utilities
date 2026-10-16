//! The native command boundary. Only service managers, account tools, the
//! broker's account resolver and the credential helper sit behind it.
//! Files are always real.

use anyhow::Result;
use secrecy::SecretString;
use std::fs::File;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Query<'a> {
    SystemdVersion,
    UnitShow,
    Passwd(&'a str),
    Shadow(&'a str),
    PasswdAll,
    /// The broker's own account resolver, answered as `uid:gid`.
    Resolve(&'a str),
    ResolveUid(u32),
    LaunchdPrint,
    LaunchdDisabled,
    DsUser(&'a str),
    DsUids,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Text(String),
    NotFound,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    DaemonReload,
    EnableUnit,
    StartUnit,
    CreateHostKey,
    CreateLinuxAccount {
        gid: u32,
        shell: String,
    },
    /// `scratch` is the installer's private directory for the import record.
    CreateMacAccount {
        uid: u32,
        nobody_gid: i64,
        scratch: PathBuf,
    },
    LaunchdEnable,
    LaunchdBootstrap,
    LaunchdKickstart,
}

pub(crate) trait Host {
    fn query(&mut self, query: Query<'_>) -> Answer;
    fn act(&mut self, action: &Action) -> Result<()>;
    /// Encrypt the token with the host key into `out`. Plaintext moves only
    /// through a private pipe, and no helper output is captured or printed.
    fn encrypt(&mut self, token: &SecretString, out: &mut File) -> Result<()>;
}
