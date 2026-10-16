//! Linux native text: systemd version and unit, PAM service, account records.

use super::facts::{AccountRecord, Fact, Service};
use super::layout::{Backend, Layout, CREDENTIAL_NAME, UNIT_NAME};
use crate::broker::identity::HUSHRUN_ACCOUNT;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PamLayout {
    /// Debian and Ubuntu: `common-auth` and `common-account`.
    Debian,
    /// Fedora, Arch and others: one `system-auth` stack.
    SystemAuth,
}

/// The major version from the first line of `systemctl --version`.
pub(crate) fn systemd_version(text: &str) -> Option<u32> {
    text.lines()
        .next()?
        .strip_prefix("systemd ")?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// systemd 250 added encrypted credentials. 247 added LoadCredential.
pub(crate) fn backend_for(version: Fact<u32>) -> Fact<Backend> {
    match version {
        Fact::Known(250..) => Fact::Known(Backend::Encrypted),
        Fact::Known(247..=249) => Fact::Known(Backend::LoadCredential),
        Fact::Known(older) => Fact::Refused(format!(
            "systemd {older} has no LoadCredential, hush needs 247 or newer"
        )),
        Fact::Refused(why) => Fact::Refused(why),
        Fact::Unknown(why) => Fact::Unknown(why),
    }
}

pub(crate) fn render_unit(
    layout: &Layout,
    backend: Backend,
    bws: &Path,
) -> Result<Vec<u8>, String> {
    let directive = match backend {
        Backend::Encrypted => "LoadCredentialEncrypted",
        Backend::LoadCredential => "LoadCredential",
        Backend::Launchd => return Err("launchd has no systemd unit".into()),
    };
    let command = layout.daemon_args(backend, bws)?.join(" ");
    let credential = layout.credential(backend);
    Ok(format!(
        "# Written by hush install. hush install refuses to adopt this unit once it changes.\n\
[Unit]\nDescription=hush secret broker daemon\nAfter=network-online.target\nWants=network-online.target\n\n\
[Service]\nType=simple\nExecStart={command}\n{directive}={CREDENTIAL_NAME}:{credential}\nRestart=on-failure\nRestartSec=5\n\n\
[Install]\nWantedBy=multi-user.target\n"
    )
    .into_bytes())
}

pub(crate) fn render_pam(pam: PamLayout) -> Vec<u8> {
    let stack = match pam {
        PamLayout::Debian => "@include common-auth\n@include common-account\n",
        PamLayout::SystemAuth => "auth include system-auth\naccount include system-auth\n",
    };
    format!("# Written by hush install. Password approval for the hush broker uses the distro stack.\n{stack}").into_bytes()
}

/// Read whitelisted `systemctl show` fields. A new or missing field is Unknown.
pub(crate) fn parse_unit_show(text: &str, fragment: &str) -> Fact<Service> {
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    };
    let fields = (
        field("LoadState"),
        field("ActiveState"),
        field("UnitFileState"),
        field("FragmentPath"),
        field("DropInPaths"),
        field("Names"),
        field("NeedDaemonReload"),
        field("MainPID").and_then(|pid| pid.parse::<i32>().ok()),
    );
    let (
        Some(load),
        Some(active),
        Some(file),
        Some(loaded_from),
        Some(dropins),
        Some(names),
        Some(reload),
        Some(pid),
    ) = fields
    else {
        return Fact::Unknown("systemctl show gave an unexpected reply".into());
    };
    match load {
        "not-found" if loaded_from.is_empty() => return Fact::Known(Service::default()),
        "masked" => return Fact::Refused(format!("{UNIT_NAME} is masked")),
        "loaded" => {}
        other => return Fact::Unknown(format!("{UNIT_NAME} has load state {other}")),
    }
    if loaded_from != fragment {
        return Fact::Refused(format!(
            "{UNIT_NAME} is defined by {loaded_from}, not {fragment}"
        ));
    }
    if !dropins.is_empty() {
        return Fact::Refused(format!("{UNIT_NAME} has drop-ins at {dropins}"));
    }
    if names.split_whitespace().ne([UNIT_NAME]) {
        return Fact::Refused(format!("{UNIT_NAME} also has the names {names}"));
    }
    // The rendered unit sets no identity, so the daemon runs as root.
    let (Some(user), Some(group), Some(dynamic)) =
        (field("User"), field("Group"), field("DynamicUser"))
    else {
        return Fact::Unknown(format!(
            "systemctl show did not report who {UNIT_NAME} runs as"
        ));
    };
    if (user, group, dynamic) != ("", "", "no") {
        return Fact::Refused(format!(
            "{UNIT_NAME} runs as User={user} Group={group} DynamicUser={dynamic}, not as root"
        ));
    }
    Fact::Known(Service {
        loaded: true,
        needs_reload: reload == "yes",
        enabled: file == "enabled",
        active: active == "active",
        pid: (pid > 0).then_some(pid),
    })
}

/// Command lists the rendered unit leaves empty. A start runs the condition
/// and pre and post commands, and a started service runs the stop lists when
/// it ends. Install never reloads the service, so ExecReload is not read.
pub(crate) const UNRENDERED_COMMANDS: [&str; 5] = [
    "ExecCondition",
    "ExecStartPre",
    "ExecStartPost",
    "ExecStop",
    "ExecStopPost",
];

/// Check the command systemd loaded, from `busctl --json=short get-property`
/// of `ExecStart`, against the rendered daemon command. Exactly one command,
/// the same executable and argv, and failures not ignored.
pub(crate) fn loaded_command(json: &str, args: &[String]) -> Fact<()> {
    match commands(json).as_deref() {
        None | Some([]) => {
            Fact::Unknown(format!("the loaded command of {UNIT_NAME} is unreadable"))
        }
        Some([(path, argv, false)]) if args.first() == Some(path) && argv == args => {
            Fact::Known(())
        }
        Some(_) => Fact::Refused(format!(
            "{UNIT_NAME} has a loaded command that is not the hush daemon command"
        )),
    }
}

/// Check that a list the rendered unit leaves empty holds no loaded command.
pub(crate) fn no_command(json: &str, list: &str) -> Fact<()> {
    match commands(json).as_deref() {
        None => Fact::Unknown(format!("the loaded {list} of {UNIT_NAME} is unreadable")),
        Some([]) => Fact::Known(()),
        Some(_) => Fact::Refused(format!(
            "{UNIT_NAME} has a loaded {list} command that the hush unit does not define"
        )),
    }
}

/// The executable, argv and ignore flag of each command in one reply.
fn commands(json: &str) -> Option<Vec<(String, Vec<String>, bool)>> {
    use serde_json::Value;
    let reply: Value = serde_json::from_str(json).ok()?;
    if reply["type"] != "a(sasbttttuii)" {
        return None;
    }
    let text = |value: &Value| value.as_str().map(String::from);
    reply["data"]
        .as_array()?
        .iter()
        .map(|command| {
            let argv: Option<_> = command[1].as_array()?.iter().map(text).collect();
            Some((text(&command[0])?, argv?, command[2].as_bool()?))
        })
        .collect()
}

/// The hushrun record from one `getent passwd` line and one `getent shadow`
/// line. A locked (`!`) or absent (`*`) password hash disables password login.
pub(crate) fn account_record(passwd: &str, shadow: &str) -> Option<AccountRecord> {
    let entry: Vec<&str> = passwd.lines().next()?.split(':').collect();
    let hash = shadow.lines().next()?.split(':').collect::<Vec<_>>();
    let ([name, _, uid, gid, _, home, shell], [shadow_name, hash, ..]) =
        (entry.as_slice(), hash.as_slice())
    else {
        return None;
    };
    (*name == HUSHRUN_ACCOUNT && *shadow_name == HUSHRUN_ACCOUNT).then_some(())?;
    Some(AccountRecord {
        uid: uid.parse().ok()?,
        gid: gid.parse().ok()?,
        home: (*home).into(),
        shell: (*shell).into(),
        locked: hash.starts_with('!') || hash.starts_with('*'),
    })
}

/// How many `getent passwd` entries carry `uid`.
pub(crate) fn uid_count(passwd_enumeration: &str, uid: u32) -> usize {
    let uid = uid.to_string();
    passwd_enumeration
        .lines()
        .filter(|line| line.split(':').nth(2) == Some(uid.as_str()))
        .count()
}
