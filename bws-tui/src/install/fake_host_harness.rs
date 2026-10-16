//! A fake native boundary over a small modelled system. Files stay real.
//! Queries answer in each tool's captured output format and actions change
//! the model. Starting the service binds a real listener, so reachability
//! still reads real kernel peer credentials.

use super::facts::{AccountRecord, Service};
use super::host::{Action, Answer, Host, Query};
use super::layout::Platform;
use anyhow::{bail, Result};
use secrecy::{ExposeSecret, SecretString};
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::rc::Rc;

pub(crate) const CIPHERTEXT: &[u8] = b"FAKE-SYSTEMD-CREDENTIAL\n";

/// Runs after each successful action, for example to race the next step.
pub(crate) type ActHook = Box<dyn FnMut(&Action)>;

/// The executable and argv systemd last loaded. Shared so a hook can change it mid-apply.
pub(crate) type Loaded = Rc<RefCell<Option<(String, Vec<String>)>>>;

pub(crate) struct FakeHost {
    pub root: PathBuf,
    pub platform: Platform,
    pub systemd: u32,
    pub account: Option<AccountRecord>,
    pub shadow_readable: bool,
    pub service: Service,
    pub disabled: Option<bool>,
    pub used_uids: Vec<i64>,
    pub actions: Vec<Action>,
    pub fail_at: Option<usize>,
    pub fail_encrypt: bool,
    pub encrypted: usize,
    pub on_act: Option<ActHook>,
    pub loaded_command: Loaded,
    /// `User=` of the loaded unit. None models a reply without the field.
    pub user: Option<String>,
    /// Commands the manager still holds in lists the rendered unit leaves
    /// empty, such as from an older definition. None models a failed query.
    pub cached: Vec<(&'static str, Option<Vec<String>>)>,
    launchd_args: Vec<String>,
    listener: Option<UnixListener>,
}

impl FakeHost {
    pub(crate) fn new(root: &std::path::Path, platform: Platform) -> Self {
        Self {
            root: root.to_path_buf(),
            platform,
            systemd: 255,
            account: None,
            shadow_readable: true,
            service: Service::default(),
            disabled: None,
            used_uids: vec![0, -2, 70, 278, 499],
            actions: Vec::new(),
            fail_at: None,
            fail_encrypt: false,
            encrypted: 0,
            on_act: None,
            loaded_command: Loaded::default(),
            user: Some(String::new()),
            cached: Vec::new(),
            launchd_args: Vec::new(),
            listener: None,
        }
    }

    fn start(&mut self) {
        self.service.active = true;
        self.service.pid = Some(std::process::id() as i32);
        let socket = match self.platform {
            Platform::Linux => self.root.join("run/hush/hush.sock"),
            // The /var alias keeps the path inside the 104-byte macOS socket limit.
            Platform::MacOs => self.root.join("var/run/hush/hush.sock"),
        };
        fs::create_dir_all(socket.parent().unwrap()).unwrap();
        self.listener = Some(UnixListener::bind(socket).unwrap());
    }

    /// The service stops on its own, for example after a crash between enable and start.
    pub(crate) fn stop(&mut self) {
        self.service.active = false;
        self.service.pid = None;
        self.listener = None;
    }

    /// The identity fields `systemctl show` prints for every unit, loaded or not.
    fn identity(&self) -> String {
        let user = self.user.as_ref().map(|user| format!("User={user}\n"));
        format!("{}Group=\nDynamicUser=no\n", user.unwrap_or_default())
    }

    fn passwd_line(&self) -> Option<String> {
        let a = self.account.as_ref()?;
        Some(format!(
            "hushrun:x:{}:{}::{}:{}\n",
            a.uid, a.gid, a.home, a.shell
        ))
    }
}

impl Host for FakeHost {
    fn query(&mut self, query: Query<'_>) -> Answer {
        let text = |text: String| Answer::Text(text);
        let account = self.account.clone();
        match query {
            Query::SystemdVersion => text(format!("systemd {} (fake)\n+PAM\n", self.systemd)),
            Query::UnitShow if !self.service.loaded => text(format!("MainPID=0\nNames=hush.service\nLoadState=not-found\nActiveState=inactive\nSubState=dead\nFragmentPath=\nDropInPaths=\nUnitFileState=\nNeedDaemonReload=no\n{}", self.identity())),
            Query::UnitShow => {
                let s = self.service;
                text(format!(
                    "MainPID={}\nNames=hush.service\nLoadState=loaded\nActiveState={}\nSubState=x\nFragmentPath=/etc/systemd/system/hush.service\nDropInPaths=\nUnitFileState={}\nNeedDaemonReload=no\n{}",
                    s.pid.unwrap_or(0),
                    if s.active { "active" } else { "inactive" },
                    if s.enabled { "enabled" } else { "disabled" },
                    self.identity()
                ))
            }
            Query::UnitCommands(list) => {
                let command = |path: &String, argv: &Vec<String>| serde_json::json!([[path, argv, false, 0, 0, 0, 0, 0, 0, 0]]);
                let data = match (list, self.cached.iter().find(|(name, _)| *name == list)) {
                    ("ExecStart", _) => match &*self.loaded_command.borrow() {
                        Some((path, argv)) => command(path, argv),
                        None => serde_json::json!([]),
                    },
                    (_, Some((_, None))) => return Answer::Failed("busctl get-property failed".into()),
                    (_, Some((_, Some(argv)))) => command(&argv[0], argv),
                    (_, None) => serde_json::json!([]),
                };
                text(serde_json::json!({"type": "a(sasbttttuii)", "data": data}).to_string())
            }
            Query::Passwd("nobody") => text("nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n".into()),
            Query::Passwd(_) => self.passwd_line().map_or(Answer::NotFound, text),
            Query::Shadow(_) if !self.shadow_readable => Answer::Failed("getent shadow: permission denied".into()),
            Query::Shadow(_) => account.map_or(Answer::NotFound, |a| text(format!("hushrun:{}:20000::::::\n", if a.locked { "!" } else { "$6$x$y" }))),
            Query::PasswdAll => text(format!("root:x:0:0::/root:/bin/sh\nnobody:x:65534:65534::/nonexistent:/usr/sbin/nologin\n{}", self.passwd_line().unwrap_or_default())),
            Query::Resolve("nobody") if self.platform == Platform::MacOs => text("4294967294:4294967294".into()),
            Query::Resolve("nobody") => text("65534:65534".into()),
            Query::Resolve(_) => account.map_or(Answer::NotFound, |a| text(format!("{}:{}", a.uid, a.gid))),
            Query::ResolveUid(uid) if self.used_uids.contains(&i64::from(uid)) => text("taken".into()),
            Query::ResolveUid(_) => Answer::NotFound,
            Query::LaunchdPrint if !self.service.loaded => Answer::NotFound,
            Query::LaunchdPrint => {
                let arguments: String = self.launchd_args.iter().map(|arg| format!("\t\t{arg}\n")).collect();
                let pid = self.service.pid.map(|pid| format!("\tpid = {pid}\n")).unwrap_or_default();
                let state = if self.service.active { "running" } else { "not running" };
                let program = self.launchd_args.first().map(String::as_str).unwrap_or_default();
                text(format!("system/hush = {{\n\tpath = /Library/LaunchDaemons/hush.plist\n\tstate = {state}\n\tprogram = {program}\n\targuments = {{\n{arguments}\t}}\n\tdomain = system\n{pid}}}\n"))
            }
            Query::LaunchdDisabled => text(match self.disabled {
                Some(true) => "\tdisabled services = {\n\t\t\"hush\" => disabled\n\t}\n".into(),
                Some(false) => "\tdisabled services = {\n\t\t\"hush\" => enabled\n\t}\n".into(),
                None => "\tdisabled services = {\n\t}\n".into(),
            }),
            Query::DsUser(_) => account.map_or(Answer::NotFound, |a| {
                text(format!("NFSHomeDirectory: {}\nPassword: {}\nPrimaryGroupID: {}\nRecordName: hushrun\nUniqueID: {}\nUserShell: {}\n", a.home, if a.locked { "*" } else { "****" }, a.gid as i32, a.uid, a.shell))
            }),
            Query::DsUids => {
                let mut lines: String = self.used_uids.iter().map(|uid| format!("_user{uid}   {uid}\n")).collect();
                if let Some(a) = &self.account {
                    lines.push_str(&format!("hushrun   {}\n", a.uid));
                }
                text(lines)
            }
        }
    }

    fn act(&mut self, action: &Action) -> Result<()> {
        if self.fail_at == Some(self.actions.len()) {
            self.actions.push(action.clone());
            bail!("injected native failure");
        }
        self.actions.push(action.clone());
        match action {
            Action::DaemonReload => {
                let unit = fs::read_to_string(self.root.join("etc/systemd/system/hush.service"));
                self.service.loaded = unit.is_ok();
                // The rendered unit defines no other command list.
                self.cached.clear();
                *self.loaded_command.borrow_mut() = unit.ok().and_then(|unit| {
                    let argv: Vec<String> = unit
                        .lines()
                        .find_map(|line| line.strip_prefix("ExecStart="))?
                        .split(' ')
                        .map(String::from)
                        .collect();
                    Some((argv[0].clone(), argv))
                });
            }
            Action::EnableUnit | Action::LaunchdEnable => {
                self.service.enabled = true;
                self.disabled = self.disabled.map(|_| false);
            }
            Action::StartUnit | Action::LaunchdKickstart => self.start(),
            Action::CreateHostKey => {
                let key = self.root.join("var/lib/systemd/credential.secret");
                fs::write(&key, b"fake host key").unwrap();
                fs::set_permissions(&key, fs::Permissions::from_mode(0o400)).unwrap();
            }
            Action::CreateLinuxAccount { gid, shell } => {
                self.account = Some(AccountRecord {
                    uid: 998,
                    gid: *gid,
                    home: "/nonexistent".into(),
                    shell: shell.clone(),
                    locked: true,
                });
            }
            Action::CreateMacAccount {
                uid, nobody_gid, ..
            } => {
                self.account = Some(AccountRecord {
                    uid: *uid,
                    gid: *nobody_gid as u32,
                    home: "/var/empty".into(),
                    shell: "/usr/bin/false".into(),
                    locked: true,
                });
            }
            Action::LaunchdBootstrap => {
                let plist =
                    fs::read_to_string(self.root.join("Library/LaunchDaemons/hush.plist")).unwrap();
                self.launchd_args = plist
                    .lines()
                    .filter_map(|line| {
                        line.trim()
                            .strip_prefix("<string>")?
                            .strip_suffix("</string>")
                    })
                    .skip(1)
                    .map(String::from)
                    .collect();
                self.service.loaded = true;
                self.start();
            }
        }
        if let Some(hook) = self.on_act.as_mut() {
            hook(action);
        }
        Ok(())
    }

    fn encrypt(&mut self, token: &SecretString, out: &mut File) -> Result<()> {
        if self.fail_encrypt {
            bail!("injected encryption failure");
        }
        self.encrypted += 1;
        out.write_all(CIPHERTEXT)?;
        Ok(writeln!(
            out,
            "{} plaintext bytes",
            token.expose_secret().len()
        )?)
    }
}
