//! Run identity. Picks the uid, gid, and groups a child runs as.

use super::peercred::PeerCredentials;
use super::runspec::RunAs;
use anyhow::{bail, Result};
use std::ffi::{CStr, CString};

/// Name of the dedicated run account. `hush install` (PR 4) creates it.
pub(crate) const HUSHRUN_ACCOUNT: &str = "hushrun";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Account {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub uid: u32,
    pub gid: u32,
    pub groups: Vec<u32>,
}

pub(crate) trait AccountDb: Send + Sync {
    fn by_name(&self, name: &str) -> Result<Option<Account>>;
    fn by_uid(&self, uid: u32) -> Result<Option<Account>>;
    fn groups(&self, account: &Account) -> Result<Vec<u32>>;
}

/// Pick the identity for one run. Root is refused whatever the source.
pub(crate) fn resolve_identity(
    peer: PeerCredentials,
    run_as: RunAs,
    accounts: &dyn AccountDb,
) -> Result<Identity> {
    let identity = match run_as {
        RunAs::Requester => match accounts.by_uid(peer.uid)? {
            Some(account) => Identity {
                uid: peer.uid,
                gid: peer.gid,
                groups: accounts.groups(&account)?,
            },
            None => Identity {
                uid: peer.uid,
                gid: peer.gid,
                groups: vec![peer.gid],
            },
        },
        RunAs::HushRun => {
            let Some(account) = accounts.by_name(HUSHRUN_ACCOUNT)? else {
                bail!(
                    "daemon policy requires the '{HUSHRUN_ACCOUNT}' account for this run, \
but no such account exists on this machine. Refusing to run"
                );
            };
            Identity {
                uid: account.uid,
                gid: account.gid,
                groups: vec![account.gid],
            }
        }
    };
    if identity.uid == 0 || identity.gid == 0 {
        bail!("hush run never runs a child as root. The request resolved to uid 0 or gid 0");
    }
    Ok(identity)
}

pub(crate) struct SystemAccounts;

impl SystemAccounts {
    fn lookup(
        &self,
        by: impl Fn(*mut libc::passwd, &mut [libc::c_char], *mut *mut libc::passwd) -> i32,
    ) -> Result<Option<Account>> {
        let mut buffer = vec![0 as libc::c_char; 16 * 1024];
        // SAFETY: zeroed passwd is a valid out buffer for the reentrant lookups.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let rc = by(&mut entry, &mut buffer, &mut found);
        if rc != 0 {
            return Err(std::io::Error::from_raw_os_error(rc).into());
        }
        if found.is_null() {
            return Ok(None);
        }
        // SAFETY: on success pw_name points into `buffer`, which is still alive.
        let name = unsafe { CStr::from_ptr(entry.pw_name) }
            .to_string_lossy()
            .into_owned();
        Ok(Some(Account {
            name,
            uid: entry.pw_uid,
            gid: entry.pw_gid,
        }))
    }
}

impl AccountDb for SystemAccounts {
    fn by_name(&self, name: &str) -> Result<Option<Account>> {
        let name = CString::new(name)?;
        self.lookup(|entry, buffer, found| {
            // SAFETY: every pointer is valid for the call and the length matches the buffer.
            unsafe {
                libc::getpwnam_r(
                    name.as_ptr(),
                    entry,
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    found,
                )
            }
        })
    }

    fn by_uid(&self, uid: u32) -> Result<Option<Account>> {
        self.lookup(|entry, buffer, found| {
            // SAFETY: every pointer is valid for the call and the length matches the buffer.
            unsafe { libc::getpwuid_r(uid, entry, buffer.as_mut_ptr(), buffer.len(), found) }
        })
    }

    // `GroupId` is `gid_t` on Linux and `int` on macOS, so the cast is real on one of them.
    #[allow(clippy::unnecessary_cast)]
    fn groups(&self, account: &Account) -> Result<Vec<u32>> {
        let name = CString::new(account.name.as_str())?;
        let mut count: libc::c_int = 64;
        loop {
            let mut groups = vec![0 as GroupId; count as usize];
            let mut wanted = count;
            // SAFETY: `groups` holds `wanted` entries; getgrouplist updates `wanted`.
            let rc = unsafe {
                libc::getgrouplist(
                    name.as_ptr(),
                    account.gid as GroupId,
                    groups.as_mut_ptr(),
                    &mut wanted,
                )
            };
            if rc >= 0 {
                groups.truncate(wanted.max(0) as usize);
                return Ok(groups.into_iter().map(|gid| gid as u32).collect());
            }
            if count >= 65_536 {
                bail!("too many groups for account {}", account.name);
            }
            count = wanted.max(count * 2);
        }
    }
}

/// getgrouplist takes `int` group ids on macOS and `gid_t` on Linux.
#[cfg(target_os = "macos")]
type GroupId = libc::c_int;
#[cfg(not(target_os = "macos"))]
type GroupId = libc::gid_t;
