//! The native boundary. Every question to the account database or service
//! manager and every native change goes through this trait, so tests replace
//! only the native side while every file stays real.

use super::model::{AccountRequest, Fact, Manager, NativeAccount};
use anyhow::Result;
use secrecy::SecretString;
use std::fmt;
use std::fs::File;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operation {
    CreateAccount(AccountRequest),
    CreateHostKey,
    /// Linux reloads the manager. macOS bootstraps the job.
    Register,
    Enable,
    Start,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CreateAccount(_) => "create the hushrun account",
            Self::CreateHostKey => "create the systemd credential host key",
            Self::Register => "register the hush service",
            Self::Enable => "enable the hush service",
            Self::Start => "start the hush service",
        })
    }
}

pub(crate) trait Host {
    /// Linux only. macOS never asks.
    fn systemd_version(&mut self) -> Fact<u32>;
    fn manager(&mut self) -> Fact<Manager>;
    fn account(&mut self) -> Fact<Option<NativeAccount>>;
    /// The nobody account's primary group, which the run account shares.
    fn nobody_gid(&mut self) -> Fact<u32>;
    /// Every uid the account database uses. macOS picks the run uid from it.
    fn used_uids(&mut self) -> Fact<Vec<u32>>;
    fn perform(&mut self, operation: &Operation) -> Result<()>;
    /// Encrypt the token into `out`. The helper gets it only through a pipe.
    fn encrypt(&mut self, token: &SecretString, out: &mut File) -> Result<()>;
}
