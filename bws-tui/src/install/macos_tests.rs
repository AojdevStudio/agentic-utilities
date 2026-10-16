//! macOS native text against captured launchctl and dscl output.

use super::custody::Owner;
use super::facts::{AccountRecord, Fact, Service};
use super::layout::{Backend, Layout, Platform};
use super::macos::{
    account_record, disabled_override, dsimport_record, free_system_uid, listed_uids, parse_print,
    render_plist,
};
use std::path::Path;

const PLIST: &str = "/Library/LaunchDaemons/hush.plist";
const MISSING: &str = "Bad request.\nCould not find service \"hush\" in domain for system\n";

fn args() -> Vec<String> {
    Layout::in_root(Path::new("/"), Owner(0), Platform::MacOs)
        .daemon_args(Backend::Launchd, Path::new("/usr/local/bin/bws"))
        .unwrap()
}

fn print(path: &str, state: &str, pid: Option<u32>, args: &[String]) -> String {
    let arguments: String = args.iter().map(|arg| format!("\t\t{arg}\n")).collect();
    let pid = pid
        .map(|pid| format!("\tpid = {pid}\n"))
        .unwrap_or_default();
    format!("system/hush = {{\n\tactive count = 1\n\tpath = {path}\n\ttype = LaunchDaemon\n\tstate = {state}\n\n\tprogram = {}\n\targuments = {{\n{arguments}\t}}\n\n\tdomain = system\n{pid}\tendpoints = {{\n\t\t\"x\" = {{\n\t\t\tport = 0x13c03\n\t\t}}\n\t}}\n}}\n", args[0])
}

#[test]
fn the_plist_runs_the_shared_daemon_arguments_under_label_hush() {
    let layout = Layout::in_root(Path::new("/"), Owner(0), Platform::MacOs);
    let plist =
        String::from_utf8(render_plist(&layout, Path::new("/usr/local/bin/bws")).unwrap()).unwrap();
    assert!(
        plist.contains("<key>Label</key>\n\t<string>hush</string>"),
        "{plist}"
    );
    let strings: Vec<&str> = plist
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("<string>")?
                .strip_suffix("</string>")
        })
        .collect();
    assert_eq!(
        &strings[1..],
        args().iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
    assert!(render_plist(&layout, Path::new("/opt/my tools/bws")).is_err());
}

#[test]
fn launchctl_print_separates_missing_ours_and_foreign_jobs() {
    assert_eq!(
        parse_print(MISSING, PLIST, &args()),
        Fact::Known(Service::default())
    );
    let running = print(PLIST, "running", Some(188), &args());
    let expected = Service {
        loaded: true,
        needs_reload: false,
        enabled: true,
        active: true,
        pid: Some(188),
    };
    assert_eq!(parse_print(&running, PLIST, &args()), Fact::Known(expected));
    let idle = print(PLIST, "not running", None, &args());
    assert_eq!(
        parse_print(&idle, PLIST, &args()),
        Fact::Known(Service {
            loaded: true,
            enabled: true,
            ..Service::default()
        })
    );
    let mut other = args();
    other[3] = "/tmp/elsewhere.sock".into();
    assert!(matches!(
        parse_print(
            &print(
                "/Library/LaunchDaemons/other.plist",
                "running",
                Some(1),
                &args()
            ),
            PLIST,
            &args()
        ),
        Fact::Refused(_)
    ));
    assert!(matches!(
        parse_print(&print(PLIST, "running", Some(1), &other), PLIST, &args()),
        Fact::Refused(_)
    ));
    assert!(matches!(
        parse_print("system/hush = {\n}\n", PLIST, &args()),
        Fact::Unknown(_)
    ));
}

#[test]
fn disabled_overrides_are_read_for_label_hush_only() {
    let text = "\tdisabled services = {\n\t\t\"com.apple.ftpd\" => disabled\n\t\t\"hush\" => disabled\n\t}\n";
    assert_eq!(disabled_override(text), Some(true));
    assert_eq!(
        disabled_override(&text.replace("\"hush\" => disabled", "\"hush\" => enabled")),
        Some(false)
    );
    assert_eq!(
        disabled_override("\tdisabled services = {\n\t\t\"com.apple.ftpd\" => disabled\n\t}\n"),
        None
    );
}

#[test]
fn directory_records_and_free_system_uids() {
    let dscl = "NFSHomeDirectory: /var/empty\nPassword: *\nPrimaryGroupID: -2\nRecordName: hushrun\nUniqueID: 498\nUserShell: /usr/bin/false\n";
    let record = AccountRecord {
        uid: 498,
        gid: u32::MAX - 1,
        home: "/var/empty".into(),
        shell: "/usr/bin/false".into(),
        locked: true,
    };
    assert_eq!(account_record(dscl), Some(record));
    assert!(
        !account_record(&dscl.replace("Password: *", "Password: ********"))
            .unwrap()
            .locked
    );
    assert_eq!(
        account_record("No such key: UniqueID\nRecordName: hushrun\n"),
        None
    );
    let list = "_accessoryupdater        278\n_amavisd                 83\nnobody                   -2\n_hidden                  499\n";
    assert_eq!(listed_uids(list), [278, 83, -2, 499]);
    assert_eq!(free_system_uid(&[278, 83, -2, 499]), Some(498));
    assert_eq!(free_system_uid(&(200..=499).collect::<Vec<_>>()), None);
    let record = dsimport_record(498, -2);
    let lines: Vec<&str> = record.lines().collect();
    assert_eq!(lines.len(), 2, "{record}");
    assert_eq!(lines[1], "hushrun:498:-2:/var/empty:/usr/bin/false:*:1");
}

/// Top-level lines of a real macOS 26.7 `launchctl print system/<label>` reply
/// for a running LaunchDaemon, relabeled for hush. launchd prints `username` and
/// `group` after `domain`, and only for a job that runs as another user.
fn cached(program: &str, identity: &str) -> String {
    let arguments: String = args().iter().map(|arg| format!("\t\t{arg}\n")).collect();
    format!("system/hush = {{\n\tactive count = 1\n\tpath = {PLIST}\n\ttype = LaunchDaemon\n\tstate = running\n\n\tprogram = {program}\n\targuments = {{\n{arguments}\t}}\n\n\tdomain = system\n{identity}\tminimum runtime = 10\n\texit timeout = 5\n\truns = 1\n\tpid = 188\n\tlast exit code = (never exited)\n\n\tspawn type = daemon (3)\n\tproperties = runatload | inferred program\n}}\n")
}

#[test]
fn cached_jobs_must_prove_the_rendered_program_and_root_identity() {
    let ours = args()[0].clone();
    let known = |text: &str| matches!(parse_print(text, PLIST, &args()), Fact::Known(_));
    let refused = |text: &str| matches!(parse_print(text, PLIST, &args()), Fact::Refused(_));
    let unknown = |text: &str| matches!(parse_print(text, PLIST, &args()), Fact::Unknown(_));
    // launchd omits username and group for root, by default or set explicitly.
    assert!(known(&cached(&ours, "")));
    assert!(known(&cached(&ours, "\tusername = root\n")));
    // Matching argv does not prove the executable or the user.
    assert!(refused(&cached("/usr/bin/false", "")));
    assert!(refused(&cached(
        &ours,
        "\tusername = _timed\n\tgroup = _timed\n"
    )));
    assert!(refused(&cached(&ours, "\tgroup = staff\n")));
    // Missing, unreadable or forged proof is unknown, never root.
    let real = cached(&ours, "");
    assert!(unknown(&real.replace(&format!("\tprogram = {ours}\n"), "")));
    assert!(unknown(&real.replace("\tdomain = system\n", "")));
    assert!(unknown(&cached(&ours, "\tusername = \n")));
    assert!(unknown(&cached(
        &ours,
        "\tusername = _timed\n\tusername = root\n"
    )));
    assert!(unknown(real.trim_end().trim_end_matches('}')));
}
