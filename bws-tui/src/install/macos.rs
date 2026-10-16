//! macOS native text: LaunchDaemon plist, launchctl replies, directory records.

use super::facts::{AccountRecord, Fact, Service};
use super::layout::{Backend, Layout, LAUNCHD_LABEL};
use crate::broker::identity::HUSHRUN_ACCOUNT;
use std::path::Path;

pub(crate) const HOME: &str = "/var/empty";
pub(crate) const SHELL: &str = "/usr/bin/false";
/// Local system account range searched downward for a free uid. A local
/// choice, not a reserved namespace. Live records are rechecked before use.
const SYSTEM_UIDS: std::ops::RangeInclusive<u32> = 200..=499;

pub(crate) fn render_plist(layout: &Layout, bws: &Path) -> Result<Vec<u8>, String> {
    let strings: String = layout
        .daemon_args(Backend::Launchd, bws)?
        .iter()
        .map(|arg| format!("\t\t<string>{arg}</string>\n"))
        .collect();
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\t<key>Label</key>\n\t<string>{LAUNCHD_LABEL}</string>\n\
\t<key>ProgramArguments</key>\n\t<array>\n{strings}\t</array>\n\t<key>RunAtLoad</key>\n\t<true/>\n\
\t<key>KeepAlive</key>\n\t<dict>\n\t\t<key>SuccessfulExit</key>\n\t\t<false/>\n\t</dict>\n</dict>\n</plist>\n"
    )
    .into_bytes())
}

/// Top-level fields read from `launchctl print system/hush`.
const FIELDS: [&str; 7] = [
    "path", "state", "program", "domain", "username", "group", "pid",
];

/// Every top-level `key = value` line. A value with an embedded newline can
/// print a second one, so callers refuse to pick between duplicates.
fn top_values<'t>(text: &'t str, key: &'t str) -> impl Iterator<Item = &'t str> {
    text.lines().filter_map(move |line| {
        line.strip_prefix('\t')?
            .strip_prefix(key)?
            .strip_prefix(" =")
            .map(|value| value.strip_prefix(' ').unwrap_or(value))
    })
}

/// Read whitelisted top-level fields of `launchctl print system/hush`.
/// Boot policy comes from `disabled_override`, so `enabled` here only means loaded.
/// The cached job must run the rendered program as root. launchd prints
/// `program` even when it is inferred from the arguments, and prints `username`
/// and `group` after `domain` only for a job that runs as another user.
pub(crate) fn parse_print(text: &str, path: &str, args: &[String]) -> Fact<Service> {
    if text.contains("Could not find service") {
        return Fact::Known(Service::default());
    }
    if let Some(key) = FIELDS
        .iter()
        .find(|key| top_values(text, key).nth(1).is_some())
    {
        return Fact::Unknown(format!("launchctl print shows {key} more than once"));
    }
    let field = |key| top_values(text, key).next();
    let (Some(loaded_from), Some(state), Some(program), Some("system"), true) = (
        field("path"),
        field("state"),
        field("program"),
        field("domain"),
        text.lines().any(|line| line == "}"),
    ) else {
        return Fact::Unknown("launchctl print gave an unexpected reply".into());
    };
    if loaded_from != path {
        return Fact::Refused(format!(
            "launchd job {LAUNCHD_LABEL} is loaded from {loaded_from}, not {path}"
        ));
    }
    let arguments: Vec<&str> = text
        .lines()
        .skip_while(|line| *line != "\targuments = {")
        .skip(1)
        .take_while(|line| *line != "\t}")
        .map(str::trim)
        .collect();
    if arguments
        .iter()
        .copied()
        .ne(args.iter().map(String::as_str))
    {
        return Fact::Refused(format!(
            "launchd job {LAUNCHD_LABEL} runs a different command"
        ));
    }
    if args.first().map(String::as_str) != Some(program) {
        return Fact::Refused(format!(
            "launchd job {LAUNCHD_LABEL} runs {program}, not the rendered program"
        ));
    }
    for (key, root) in [("username", "root"), ("group", "wheel")] {
        match field(key) {
            None => {}
            Some(value) if value == root => {}
            Some("") => {
                return Fact::Unknown(format!(
                    "launchd job {LAUNCHD_LABEL} has an unreadable {key}"
                ))
            }
            Some(value) => {
                return Fact::Refused(format!(
                    "launchd job {LAUNCHD_LABEL} runs with {key} {value}, not {root}"
                ))
            }
        }
    }
    let active = state == "running";
    let pid = field("pid").and_then(|pid| pid.parse().ok());
    if active && pid.is_none() {
        return Fact::Unknown(format!(
            "launchd job {LAUNCHD_LABEL} runs with no readable pid"
        ));
    }
    Fact::Known(Service {
        loaded: true,
        needs_reload: false,
        enabled: true,
        active,
        pid,
    })
}

/// The disabled override for label hush from `launchctl print-disabled system`.
pub(crate) fn disabled_override(text: &str) -> Option<bool> {
    let entry = format!("\"{LAUNCHD_LABEL}\" => ");
    text.lines()
        .find_map(|line| match line.trim().strip_prefix(&entry)? {
            "disabled" | "true" => Some(true),
            "enabled" | "false" => Some(false),
            _ => None,
        })
}

/// The hushrun record from `dscl . -read`. A missing or multi-line key means
/// an unexplained record, so it yields None.
pub(crate) fn account_record(dscl: &str) -> Option<AccountRecord> {
    let field = |key: &str| {
        dscl.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
    };
    (field("RecordName")?.split_whitespace().next()? == HUSHRUN_ACCOUNT).then_some(())?;
    let gid: i64 = field("PrimaryGroupID")?.parse().ok()?;
    Some(AccountRecord {
        uid: field("UniqueID")?.parse().ok()?,
        // Directory gids are signed. nobody is -2, which getpwnam reports as u32.
        gid: gid as u32,
        home: field("NFSHomeDirectory")?.into(),
        shell: field("UserShell")?.into(),
        locked: field("Password")? == "*",
    })
}

/// Every uid in `dscl . -list /Users UniqueID`.
pub(crate) fn listed_uids(dscl_list: &str) -> Vec<i64> {
    dscl_list
        .lines()
        .filter_map(|line| line.split_whitespace().last()?.parse().ok())
        .collect()
}

pub(crate) fn free_system_uid(used: &[i64]) -> Option<u32> {
    SYSTEM_UIDS
        .rev()
        .find(|uid| !used.contains(&i64::from(*uid)))
}

/// One complete dsimport record: hidden, password disabled with `*`, no home, no shell.
pub(crate) fn dsimport_record(uid: u32, nobody_gid: i64) -> String {
    let attributes = [
        "RecordName",
        "UniqueID",
        "PrimaryGroupID",
        "NFSHomeDirectory",
        "UserShell",
        "Password",
        "IsHidden",
    ];
    let header: Vec<String> = attributes
        .iter()
        .map(|name| format!("dsAttrTypeStandard:{name}"))
        .collect();
    format!(
        "0x0A 0x5C 0x3A 0x2C dsRecTypeStandard:Users {} {}\n{HUSHRUN_ACCOUNT}:{uid}:{nobody_gid}:{HOME}:{SHELL}:*:1\n",
        attributes.len(),
        header.join(" ")
    )
}
