//! The production native boundary. Fixed absolute programs whose custody is
//! checked before first use, a cleared environment, no stdin unless the
//! credential helper needs the token, bounded stdout, discarded stderr and a
//! deadline. No helper output ever reaches an error or the terminal.

use super::custody::{Dir, Owner};
use super::facts::Fact;
use super::host::{Action, Answer, Host, Query};
use super::layout::{rel, CREDENTIAL_NAME, LAUNCHD_LABEL, UNIT_NAME};
use super::sys::create_exclusive;
use crate::broker::identity::{AccountDb, SystemAccounts, HUSHRUN_ACCOUNT};
use anyhow::{bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const SYSTEMD_CREDS: &str = "/usr/bin/systemd-creds";
const GETENT: &str = "/usr/bin/getent";
const USERADD: &str = "/usr/sbin/useradd";
const LAUNCHCTL: &str = "/bin/launchctl";
const DSCL: &str = "/usr/bin/dscl";
const DSIMPORT: &str = "/usr/bin/dsimport";
const PLIST: &str = "/Library/LaunchDaemons/hush.plist";
const SHOW_FIELDS: &str =
    "LoadState,ActiveState,UnitFileState,FragmentPath,DropInPaths,MainPID,Names,NeedDaemonReload";
const OUTPUT_LIMIT: u64 = 1 << 20;
const DEADLINE: Duration = Duration::from_secs(120);

#[derive(Default)]
pub(crate) struct NativeHost;

impl Host for NativeHost {
    fn query(&mut self, query: Query<'_>) -> Answer {
        let (program, args, not_found): (&str, Vec<String>, Option<i32>) = match query {
            Query::Resolve(name) => return resolved(SystemAccounts.by_name(name)),
            Query::ResolveUid(uid) => return resolved(SystemAccounts.by_uid(uid)),
            // A nonroot getent cannot read shadow and reports the entry as missing.
            // SAFETY: geteuid takes no arguments and cannot fail.
            Query::Shadow(_) if unsafe { libc::geteuid() } != 0 => {
                return Answer::Failed("reading account password state needs root".into())
            }
            Query::SystemdVersion => (SYSTEMCTL, vec!["--version".into()], None),
            Query::UnitShow => (
                SYSTEMCTL,
                vec![
                    "show".into(),
                    UNIT_NAME.into(),
                    "-p".into(),
                    SHOW_FIELDS.into(),
                ],
                None,
            ),
            Query::Passwd(name) => (GETENT, vec!["passwd".into(), name.into()], Some(2)),
            Query::Shadow(name) => (GETENT, vec!["shadow".into(), name.into()], Some(2)),
            Query::PasswdAll => (GETENT, vec!["passwd".into()], None),
            Query::LaunchdPrint => (
                LAUNCHCTL,
                vec!["print".into(), format!("system/{LAUNCHD_LABEL}")],
                Some(113),
            ),
            Query::LaunchdDisabled => (
                LAUNCHCTL,
                vec!["print-disabled".into(), "system".into()],
                None,
            ),
            Query::DsUser(name) => {
                let mut args = vec![".".into(), "-read".into(), format!("/Users/{name}")];
                args.extend(
                    [
                        "RecordName",
                        "UniqueID",
                        "PrimaryGroupID",
                        "NFSHomeDirectory",
                        "UserShell",
                        "Password",
                    ]
                    .map(String::from),
                );
                (DSCL, args, Some(56))
            }
            Query::DsUids => (
                DSCL,
                vec![
                    ".".into(),
                    "-list".into(),
                    "/Users".into(),
                    "UniqueID".into(),
                ],
                None,
            ),
        };
        match run(program, &args, None) {
            Ok((Some(0), text)) => Answer::Text(text),
            Ok((code, _)) if code.is_some() && code == not_found => Answer::NotFound,
            Ok((code, _)) => Answer::Failed(format!("{program} exited with {}", describe(code))),
            Err(error) => Answer::Failed(format!("{error:#}")),
        }
    }

    fn act(&mut self, action: &Action) -> Result<()> {
        let unit = |verb: &str| (SYSTEMCTL, vec![verb.to_string(), UNIT_NAME.into()]);
        let job = format!("system/{LAUNCHD_LABEL}");
        let (program, args) = match action {
            Action::DaemonReload => (SYSTEMCTL, vec!["daemon-reload".into()]),
            Action::EnableUnit => unit("enable"),
            Action::StartUnit => unit("start"),
            Action::CreateHostKey => (SYSTEMD_CREDS, vec!["setup".into()]),
            Action::CreateLinuxAccount { gid, shell } => (
                USERADD,
                [
                    "--system",
                    "--no-user-group",
                    "--no-create-home",
                    "--no-log-init",
                    "--gid",
                    &gid.to_string(),
                    "--home-dir",
                    "/nonexistent",
                    "--shell",
                    shell,
                    HUSHRUN_ACCOUNT,
                ]
                .map(String::from)
                .to_vec(),
            ),
            Action::CreateMacAccount {
                uid,
                nobody_gid,
                scratch,
            } => return import_account(*uid, *nobody_gid, scratch),
            Action::LaunchdEnable => (LAUNCHCTL, vec!["enable".into(), job]),
            Action::LaunchdBootstrap => (
                LAUNCHCTL,
                vec!["bootstrap".into(), "system".into(), PLIST.into()],
            ),
            Action::LaunchdKickstart => (LAUNCHCTL, vec!["kickstart".into(), job]),
        };
        finished(program, &args, run(program, &args, None)?.0)
    }

    fn encrypt(&mut self, token: &SecretString, out: &mut File) -> Result<()> {
        let args = [
            "encrypt",
            "--with-key=host",
            &format!("--name={CREDENTIAL_NAME}"),
            "-",
            "-",
        ]
        .map(String::from);
        let output = out
            .try_clone()
            .context("cannot hand the staging file to systemd-creds")?;
        let code = run(SYSTEMD_CREDS, &args, Some((token, output)))?.0;
        finished(SYSTEMD_CREDS, &args, code)
    }
}

/// Write one complete record to a private file, import it, then remove the file.
fn import_account(uid: u32, nobody_gid: i64, scratch: &Path) -> Result<()> {
    let name = format!(".hush-account-import.{}", std::process::id());
    let dir = match Dir::open_root(scratch, Owner(0)) {
        Fact::Known(dir) => dir,
        Fact::Refused(why) | Fact::Unknown(why) => bail!(
            "cannot use {} for the account record ({why})",
            scratch.display()
        ),
    };
    let mut record =
        File::from(create_exclusive(dir.fd(), &name).context("cannot stage the account record")?);
    record.write_all(super::macos::dsimport_record(uid, nobody_gid).as_bytes())?;
    record.sync_all()?;
    let path = scratch.join(&name).display().to_string();
    // --crypt makes `*` a crypt value that matches no password, not a plaintext password.
    let args = [
        path.clone(),
        "/Local/Default".into(),
        "I".into(),
        "--crypt".into(),
    ];
    let imported = run(DSIMPORT, &args, None).and_then(|(code, _)| finished(DSIMPORT, &args, code));
    std::fs::remove_file(&path).context("cannot remove the account record")?;
    imported
}

fn resolved(account: Result<Option<crate::broker::identity::Account>>) -> Answer {
    match account {
        Ok(Some(account)) => Answer::Text(format!("{}:{}", account.uid, account.gid)),
        Ok(None) => Answer::NotFound,
        Err(error) => Answer::Failed(format!("account lookup failed ({error})")),
    }
}

fn finished(program: &str, args: &[String], code: Option<i32>) -> Result<()> {
    if code == Some(0) {
        return Ok(());
    }
    bail!(
        "{program} {} exited with {}",
        args.first().map_or("", String::as_str),
        describe(code)
    )
}

fn describe(code: Option<i32>) -> String {
    code.map_or("a signal".into(), |code| format!("code {code}"))
}

/// Run one checked program. Stdout is bounded and read on its own thread, so
/// a chatty child cannot block. A child past the deadline is killed and reaped.
fn run(
    program: &str,
    args: &[String],
    input: Option<(&SecretString, File)>,
) -> Result<(Option<i32>, String)> {
    let root = match Dir::open_root(Path::new("/"), Owner(0)) {
        Fact::Known(root) => root,
        Fact::Refused(why) | Fact::Unknown(why) => bail!("cannot check {program} ({why})"),
    };
    if let Fact::Refused(why) | Fact::Unknown(why) =
        root.executable(rel(Path::new(program)), Owner(0))
    {
        bail!("{program} is not a trusted native program ({why})");
    }
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stderr(Stdio::null());
    let (stdin, stdout) = match &input {
        Some((_, out)) => (Stdio::piped(), Stdio::from(out.try_clone()?)),
        None => (Stdio::null(), Stdio::piped()),
    };
    let mut child = command
        .stdin(stdin)
        .stdout(stdout)
        .spawn()
        .with_context(|| format!("cannot run {program}"))?;
    if let Some((token, _)) = input {
        let mut pipe = child.stdin.take().context("no stdin pipe")?;
        let written = pipe.write_all(token.expose_secret().as_bytes());
        drop(pipe);
        if written.is_err() {
            reap(&mut child);
            bail!("{program} did not accept its input");
        }
    }
    let reader = child.stdout.take().map(|stdout| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout.take(OUTPUT_LIMIT).read_to_string(&mut text);
            text
        })
    });
    let deadline = Instant::now() + DEADLINE;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() > deadline {
            reap(&mut child);
            bail!(
                "{program} did not finish within {} seconds",
                DEADLINE.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let text = reader
        .map(|reader| reader.join().unwrap_or_default())
        .unwrap_or_default();
    Ok((status.code(), text))
}

fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
