//! hushrun readback. A record is Ready only when every native field, the
//! broker's own resolver and uid uniqueness agree. Anything partial or
//! unexplained is refused, never completed or adopted.

use super::custody::Dir;
use super::facts::{AccountRecord, Fact};
use super::host::{Answer, Host, Query};
use super::layout::{rel, Layout, Platform};
use super::observe::{Account, Creation};
use super::{linux, macos};
use crate::broker::identity::HUSHRUN_ACCOUNT;
use std::path::Path;

const LINUX_HOME: &str = "/nonexistent";
const LINUX_SHELL: &str = "/usr/sbin/nologin";

pub(crate) fn observe(layout: &Layout, root: &Dir, host: &mut dyn Host) -> Fact<Account> {
    let nobody_gid = match host.query(Query::Resolve("nobody")) {
        Answer::Text(text) => match text
            .trim()
            .split_once(':')
            .and_then(|(_, gid)| gid.parse::<u32>().ok())
        {
            Some(gid) if gid != 0 => gid,
            _ => return Fact::Refused("the nobody account has no nonroot primary group".into()),
        },
        Answer::NotFound => return Fact::Refused("this host has no nobody account".into()),
        Answer::Failed(why) => return Fact::Unknown(why),
    };
    match layout.platform {
        Platform::Linux => linux_account(layout, root, host, nobody_gid),
        Platform::MacOs => mac_account(host, nobody_gid),
    }
}

fn linux_account(
    layout: &Layout,
    root: &Dir,
    host: &mut dyn Host,
    nobody_gid: u32,
) -> Fact<Account> {
    if let Fact::Refused(why) | Fact::Unknown(why) =
        root.executable(rel(Path::new(LINUX_SHELL)), layout.owner)
    {
        return Fact::Refused(format!("no verified nonlogin shell ({why})"));
    }
    let shadow = host.query(Query::Shadow(HUSHRUN_ACCOUNT));
    match (
        host.query(Query::Passwd(HUSHRUN_ACCOUNT)),
        host.query(Query::Resolve(HUSHRUN_ACCOUNT)),
        shadow,
    ) {
        (Answer::NotFound, Answer::NotFound, Answer::NotFound | Answer::Failed(_)) => {
            Fact::Known(Account::Absent(Creation::Linux {
                gid: nobody_gid,
                shell: LINUX_SHELL.into(),
            }))
        }
        (Answer::Text(passwd), Answer::Text(resolved), Answer::Text(shadow)) => {
            let Some(record) = linux::account_record(&passwd, &shadow) else {
                return refused_record();
            };
            let expected = record.uid != 0
                && record.gid == nobody_gid
                && record.home == LINUX_HOME
                && record.shell == LINUX_SHELL
                && record.locked;
            let unique = match host.query(Query::PasswdAll) {
                Answer::Text(all) => linux::uid_count(&all, record.uid) == 1,
                Answer::NotFound => false,
                Answer::Failed(why) => return Fact::Unknown(why),
            };
            ready(record, expected, unique, &resolved)
        }
        (Answer::Text(_), Answer::Text(_), Answer::Failed(_)) => Fact::Unknown(format!(
            "the {HUSHRUN_ACCOUNT} password state needs root to read"
        )),
        (Answer::Failed(why), _, _) | (_, Answer::Failed(why), _) => Fact::Unknown(why),
        _ => refused_record(),
    }
}

fn mac_account(host: &mut dyn Host, nobody_gid: u32) -> Fact<Account> {
    let uids = match host.query(Query::DsUids) {
        Answer::Text(text) => macos::listed_uids(&text),
        Answer::NotFound => Vec::new(),
        Answer::Failed(why) => return Fact::Unknown(why),
    };
    match (
        host.query(Query::DsUser(HUSHRUN_ACCOUNT)),
        host.query(Query::Resolve(HUSHRUN_ACCOUNT)),
    ) {
        (Answer::NotFound, Answer::NotFound) => {
            let mut used = uids;
            loop {
                let Some(uid) = macos::free_system_uid(&used) else {
                    return Fact::Refused("no free local system uid between 200 and 499".into());
                };
                match host.query(Query::ResolveUid(uid)) {
                    Answer::NotFound => {
                        return Fact::Known(Account::Absent(Creation::Mac {
                            uid,
                            nobody_gid: i64::from(nobody_gid as i32),
                        }))
                    }
                    Answer::Text(_) => used.push(i64::from(uid)),
                    Answer::Failed(why) => return Fact::Unknown(why),
                }
            }
        }
        (Answer::Text(dscl), Answer::Text(resolved)) => {
            let Some(record) = macos::account_record(&dscl) else {
                return refused_record();
            };
            let expected = (1..500).contains(&record.uid)
                && record.gid == nobody_gid
                && record.home == macos::HOME
                && record.shell == macos::SHELL
                && record.locked;
            let unique = uids
                .iter()
                .filter(|uid| **uid == i64::from(record.uid))
                .count()
                == 1;
            ready(record, expected, unique, &resolved)
        }
        (Answer::Failed(why), _) | (_, Answer::Failed(why)) => Fact::Unknown(why),
        _ => refused_record(),
    }
}

fn ready(record: AccountRecord, expected: bool, unique: bool, resolved: &str) -> Fact<Account> {
    if !expected {
        return Fact::Refused(format!(
            "account {HUSHRUN_ACCOUNT} exists with fields hush install did not create"
        ));
    }
    if !unique {
        return Fact::Refused(format!(
            "another account shares uid {} with {HUSHRUN_ACCOUNT}",
            record.uid
        ));
    }
    if resolved.trim() != format!("{}:{}", record.uid, record.gid) {
        return Fact::Refused(format!(
            "the broker resolves {HUSHRUN_ACCOUNT} to a different identity"
        ));
    }
    Fact::Known(Account::Ready(record))
}

fn refused_record() -> Fact<Account> {
    Fact::Refused(format!(
        "account {HUSHRUN_ACCOUNT} has a partial or unexplained native record"
    ))
}
