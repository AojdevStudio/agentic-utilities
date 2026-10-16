//! Linux native text against captured systemd 255 and getent output.

use super::custody::Owner;
use super::facts::{AccountRecord, Fact, Service};
use super::layout::{Backend, Layout, Platform};
use super::linux::{
    account_record, backend_for, loaded_command, no_command, parse_unit_show, render_pam,
    render_unit, systemd_version, uid_count, PamLayout,
};
use std::path::Path;

const FRAGMENT: &str = "/etc/systemd/system/hush.service";
const ABSENT: &str = "MainPID=0\nNames=hush.service\nLoadState=not-found\nActiveState=inactive\nSubState=dead\nFragmentPath=\nDropInPaths=\nUnitFileState=\nNeedDaemonReload=no\n";

/// A `systemctl show` reply for a running hush unit, with some fields replaced.
fn show(changes: &[(&str, &str)]) -> String {
    let base = [
        ("MainPID", "812"),
        ("Names", "hush.service"),
        ("LoadState", "loaded"),
        ("ActiveState", "active"),
        ("SubState", "running"),
        ("FragmentPath", FRAGMENT),
        ("DropInPaths", ""),
        ("UnitFileState", "enabled"),
        ("NeedDaemonReload", "no"),
        ("User", ""),
        ("Group", ""),
        ("DynamicUser", "no"),
    ];
    base.iter()
        .map(|(key, value)| {
            let value = changes
                .iter()
                .find(|(changed, _)| changed == key)
                .map_or(*value, |(_, value)| value);
            format!("{key}={value}\n")
        })
        .collect()
}

fn layout() -> Layout {
    Layout::in_root(Path::new("/"), Owner(0), Platform::Linux)
}

#[test]
fn systemd_versions_choose_encrypted_load_credential_or_refusal() {
    let text = "systemd 255 (255.4-1ubuntu8.18)\n+PAM +AUDIT\n";
    assert_eq!(systemd_version(text), Some(255));
    assert_eq!(systemd_version("not systemd\n"), None);
    for (version, backend) in [
        (255, Backend::Encrypted),
        (250, Backend::Encrypted),
        (249, Backend::LoadCredential),
        (247, Backend::LoadCredential),
    ] {
        assert_eq!(
            backend_for(Fact::Known(version)),
            Fact::Known(backend),
            "{version}"
        );
    }
    assert!(matches!(backend_for(Fact::Known(246)), Fact::Refused(_)));
    assert!(matches!(
        backend_for(Fact::Unknown("query failed".into())),
        Fact::Unknown(_)
    ));
}

#[test]
fn units_name_one_credential_and_pass_only_its_path() {
    let bws = Path::new("/usr/local/bin/bws");
    let encrypted =
        String::from_utf8(render_unit(&layout(), Backend::Encrypted, bws).unwrap()).unwrap();
    let exec = "ExecStart=/usr/local/bin/hush daemon --socket /run/hush/hush.sock --token-file ${CREDENTIALS_DIRECTORY}/bws-token --audit-log /var/log/hush/audit.log --bws-path /usr/local/bin/bws --policy /etc/hush/hushd.toml";
    assert!(encrypted.lines().any(|line| line == exec), "{encrypted}");
    assert!(encrypted
        .lines()
        .any(|line| line == "LoadCredentialEncrypted=bws-token:/etc/hush/token.cred"));
    let plain =
        String::from_utf8(render_unit(&layout(), Backend::LoadCredential, bws).unwrap()).unwrap();
    assert!(plain
        .lines()
        .any(|line| line == "LoadCredential=bws-token:/etc/hush/token"));
    for unit in [&encrypted, &plain] {
        assert!(!unit.contains("Environment"), "{unit}");
        assert!(unit
            .lines()
            .any(|line| line == "WantedBy=multi-user.target"));
    }
    assert!(render_unit(
        &layout(),
        Backend::Encrypted,
        Path::new("/opt/my tools/bws")
    )
    .is_err());
}

#[test]
fn pam_services_include_the_checked_distro_stacks() {
    let debian = String::from_utf8(render_pam(PamLayout::Debian)).unwrap();
    assert_eq!(
        debian
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>(),
        ["@include common-auth", "@include common-account"]
    );
    let system = String::from_utf8(render_pam(PamLayout::SystemAuth)).unwrap();
    assert_eq!(
        system
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>(),
        ["auth include system-auth", "account include system-auth"]
    );
}

#[test]
fn unit_show_separates_absent_ours_and_foreign_definitions() {
    assert_eq!(
        parse_unit_show(ABSENT, FRAGMENT),
        Fact::Known(Service::default())
    );
    let expected = Service {
        loaded: true,
        needs_reload: false,
        enabled: true,
        active: true,
        pid: Some(812),
    };
    assert_eq!(parse_unit_show(&show(&[]), FRAGMENT), Fact::Known(expected));
    let stale = show(&[
        ("ActiveState", "inactive"),
        ("UnitFileState", "disabled"),
        ("NeedDaemonReload", "yes"),
        ("MainPID", "0"),
    ]);
    let expected = Service {
        loaded: true,
        needs_reload: true,
        enabled: false,
        active: false,
        pid: None,
    };
    assert_eq!(parse_unit_show(&stale, FRAGMENT), Fact::Known(expected));
    for foreign in [
        show(&[
            ("LoadState", "masked"),
            ("FragmentPath", ""),
            ("UnitFileState", "masked"),
        ]),
        show(&[("FragmentPath", "/usr/lib/systemd/system/hush.service")]),
        show(&[("DropInPaths", "/etc/systemd/system/hush.service.d/x.conf")]),
        show(&[("Names", "hush.service other.service")]),
    ] {
        assert!(
            matches!(parse_unit_show(&foreign, FRAGMENT), Fact::Refused(_)),
            "{foreign}"
        );
    }
    assert!(matches!(
        parse_unit_show("LoadState=loaded\n", FRAGMENT),
        Fact::Unknown(_)
    ));
}

#[test]
fn account_records_read_passwd_and_shadow_lock_state() {
    let passwd = "hushrun:x:998:65534::/nonexistent:/usr/sbin/nologin\n";
    let record = AccountRecord {
        uid: 998,
        gid: 65534,
        home: "/nonexistent".into(),
        shell: "/usr/sbin/nologin".into(),
        locked: true,
    };
    assert_eq!(
        account_record(passwd, "hushrun:!:20000::::::\n"),
        Some(record.clone())
    );
    assert_eq!(
        account_record(passwd, "hushrun:!*:20000::::::\n"),
        Some(record.clone())
    );
    let usable = account_record(passwd, "hushrun:$6$salt$hash:20000::::::\n").unwrap();
    assert!(!usable.locked);
    assert_eq!(
        account_record("other:x:1:1::/:/bin/sh\n", "hushrun:!::::::\n"),
        None
    );
    assert_eq!(account_record("hushrun:x:998\n", "hushrun:!::::::\n"), None);
    let all = "root:x:0:0::/root:/bin/bash\nhushrun:x:998:65534::/nonexistent:/usr/sbin/nologin\ntwin:x:998:1::/:/bin/sh\n";
    assert_eq!(uid_count(all, 998), 2);
    assert_eq!(uid_count(all, 0), 1);
}

#[test]
fn the_loaded_command_keeps_argument_boundaries_from_busctl() {
    // Captured from busctl --json=short on systemd 255 for a vendor unit.
    let captured = r#"{"type":"a(sasbttttuii)","data":[["sh",["sh","-c","[ \"$AUTOSCAN\" != \"true\" ] || /sbin/mdadm --monitor --oneshot --scan"],false,0,0,0,0,0,0,0]]}"#;
    let script = "[ \"$AUTOSCAN\" != \"true\" ] || /sbin/mdadm --monitor --oneshot --scan";
    let args = ["sh", "-c", script].map(String::from);
    assert_eq!(loaded_command(captured, &args), Fact::Known(()));
    let split: Vec<String> = ["sh", "-c"]
        .into_iter()
        .chain(script.split(' '))
        .map(String::from)
        .collect();
    assert!(matches!(loaded_command(captured, &split), Fact::Refused(_)));
    let absent = r#"{"type":"a(sasbttttuii)","data":[]}"#;
    assert!(matches!(loaded_command(absent, &args), Fact::Unknown(_)));
    // Captured the same way for the ExecStartPre list of a vendor unit.
    let pre = r#"{"type":"a(sasbttttuii)","data":[["/usr/sbin/sshd",["/usr/sbin/sshd","-t"],false,1792045132701340,1059766237265,1792045132709720,1059766245647,1224493,1,0]]}"#;
    assert!(matches!(no_command(pre, "ExecStartPre"), Fact::Refused(_)));
    assert_eq!(no_command(absent, "ExecStartPre"), Fact::Known(()));
    assert!(matches!(no_command("{}", "ExecStartPre"), Fact::Unknown(_)));
}
