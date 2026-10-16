//! One read-only observation shared by status and install. It never writes,
//! never reads a token and never needs root. Inaccessible facts are Unknown.

use super::custody::{Contents, Dir, FileState, Mode, Walk};
use super::facts::{AccountRecord, Fact, Service};
use super::host::{Answer, Host, Query};
use super::layout::{rel, Backend, Layout, Platform, HOST_KEY};
use super::reach::{self, Reach};
use super::{accounts, linux, macos, prereq};
use std::path::{Path, PathBuf};

/// One durable artifact: its logical path and everything needed to inspect and publish it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Artifact {
    pub path: PathBuf,
    pub spec: Spec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Spec {
    Dir(u32),
    /// Root-only 0600 token source, written from operator input, never read here.
    Credential,
    /// Owner-only host key the service manager creates.
    HostKey,
    /// Exact bytes and mode.
    Rendered(Vec<u8>, u32),
    /// A PAM service. An existing protected file is the operator's and is kept.
    /// None means no known distro layout to render from.
    Pam(Option<Vec<u8>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Account {
    Absent(Creation),
    Ready(AccountRecord),
}

/// What creating hushrun would use, observed before any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Creation {
    Linux { gid: u32, shell: String },
    Mac { uid: u32, nobody_gid: i64 },
}

pub(crate) struct Observation {
    pub layout: Layout,
    /// Prerequisites with no artifact of their own: executable, policy, aliases.
    pub checks: Vec<Fact<()>>,
    pub backend: Fact<Backend>,
    pub bws: Fact<PathBuf>,
    pub artifacts: Vec<(Artifact, Fact<FileState>)>,
    pub account: Fact<Account>,
    pub service: Fact<Service>,
    pub socket: Fact<Reach>,
}

pub(crate) fn observe(layout: Layout, host: &mut dyn Host) -> Observation {
    let root = match Dir::open_root(&layout.root, layout.owner) {
        Fact::Known(root) => root.trusting_log_parent(&layout),
        Fact::Refused(why) => return Observation::blocked(layout, Fact::Refused(why)),
        Fact::Unknown(why) => return Observation::blocked(layout, Fact::Unknown(why)),
    };
    let mut checks = prereq::checks(&layout, &root);
    let bws = prereq::resolve_bws(&layout, &root);
    let bws_path = known(&bws);
    let backend = match layout.platform {
        Platform::Linux => linux::backend_for(match host.query(Query::SystemdVersion) {
            Answer::Text(text) => linux::systemd_version(&text).map_or(
                Fact::Unknown("systemctl --version gave an unexpected reply".into()),
                Fact::Known,
            ),
            Answer::NotFound => Fact::Refused("systemd is not available".into()),
            Answer::Failed(why) => Fact::Unknown(why),
        }),
        Platform::MacOs => Fact::Known(Backend::Launchd),
    };
    let table = artifact_table(
        &layout,
        &root,
        known(&backend),
        bws_path.as_deref(),
        &mut checks,
    );
    let artifacts = table
        .into_iter()
        .map(|artifact| (inspect(&root, &layout, &artifact), artifact))
        .map(|(fact, artifact)| (artifact, fact))
        .collect();
    let account = accounts::observe(&layout, &root, host);
    let service = prereq::service(&layout, host, known(&backend), bws_path.as_deref());
    let socket = reach::probe(&layout.host_path(&layout.socket));
    Observation {
        layout,
        checks,
        backend,
        bws,
        artifacts,
        account,
        service,
        socket,
    }
}

impl Observation {
    /// The root itself could not be opened, so every fact carries that reason.
    fn blocked(layout: Layout, why: Fact<()>) -> Self {
        fn copy<T>(why: &Fact<()>) -> Fact<T> {
            match why {
                Fact::Refused(why) => Fact::Refused(why.clone()),
                Fact::Unknown(why) => Fact::Unknown(why.clone()),
                Fact::Known(()) => Fact::Unknown("the install root could not be opened".into()),
            }
        }
        Observation {
            backend: copy(&why),
            bws: copy(&why),
            artifacts: Vec::new(),
            account: copy(&why),
            service: copy(&why),
            socket: copy(&why),
            checks: vec![why],
            layout,
        }
    }
}

/// Every durable artifact in apply order. Rendered artifacts need a known
/// backend and bws. A render that cannot be written safely becomes a check.
pub(crate) fn artifact_table(
    layout: &Layout,
    root: &Dir,
    backend: Option<Backend>,
    bws: Option<&Path>,
    checks: &mut Vec<Fact<()>>,
) -> Vec<Artifact> {
    let entry = |path: &str, spec| Artifact {
        path: PathBuf::from(path),
        spec,
    };
    let mut table = vec![entry(layout.token_dir(), Spec::Dir(0o700))];
    table.extend(layout.log_dir().map(|dir| entry(dir, Spec::Dir(0o700))));
    let Some(backend) = backend else { return table };
    if backend == Backend::Encrypted {
        table.push(entry(HOST_KEY, Spec::HostKey));
    }
    table.push(entry(&layout.credential(backend), Spec::Credential));
    if layout.platform == Platform::Linux {
        table.push(entry(
            "/etc/pam.d/hush",
            Spec::Pam(pam_layout(root, layout).map(linux::render_pam)),
        ));
    }
    let Some(bws) = bws else { return table };
    let rendered = match layout.platform {
        Platform::Linux => {
            linux::render_unit(layout, backend, bws).map(|bytes| Spec::Rendered(bytes, 0o644))
        }
        Platform::MacOs => {
            macos::render_plist(layout, bws).map(|bytes| Spec::Rendered(bytes, 0o600))
        }
    };
    match rendered {
        Ok(spec) => table.push(entry(layout.service_file(), spec)),
        Err(why) => checks.push(Fact::Refused(why)),
    }
    table
}

/// The current state of one artifact, checked by descriptor from the root.
pub(crate) fn inspect(root: &Dir, layout: &Layout, artifact: &Artifact) -> Fact<FileState> {
    let path = artifact.path.display();
    if let Spec::Dir(mode) = artifact.spec {
        return match root.walk(rel(&artifact.path), layout.owner) {
            Fact::Known(Walk::Found(dir)) if dir.mode() == mode => Fact::Known(FileState::Ready),
            Fact::Known(Walk::Found(dir)) => Fact::Refused(format!(
                "{path} has mode {:o}, expected {mode:o}",
                dir.mode()
            )),
            Fact::Known(Walk::Missing { rest, .. }) if rest.len() == 1 => {
                Fact::Known(FileState::Absent)
            }
            Fact::Known(Walk::Missing { parent, .. }) => Fact::Refused(format!(
                "{path} needs a missing parent below {}",
                parent.path().display()
            )),
            Fact::Refused(why) => Fact::Refused(why),
            Fact::Unknown(why) => Fact::Unknown(why),
        };
    }
    let (Some(parent), Some(name)) = (
        artifact.path.parent(),
        artifact.path.file_name().and_then(|name| name.to_str()),
    ) else {
        return Fact::Refused(format!("{path} names no file"));
    };
    let ours = [Some(layout.token_dir()), layout.log_dir()].contains(&parent.to_str());
    let dir = match root.walk(rel(parent), layout.owner) {
        Fact::Known(Walk::Found(dir)) => dir,
        Fact::Known(Walk::Missing { rest, .. }) if ours && rest.len() == 1 => {
            return Fact::Known(FileState::Absent)
        }
        Fact::Known(Walk::Missing { .. }) => {
            return Fact::Refused(format!("{} is missing", parent.display()))
        }
        Fact::Refused(why) => return Fact::Refused(why),
        Fact::Unknown(why) => return Fact::Unknown(why),
    };
    let (mode, contents) = match &artifact.spec {
        Spec::Credential => (Mode::Exactly(0o600), Contents::Opaque),
        Spec::HostKey => (Mode::Private, Contents::Opaque),
        Spec::Rendered(bytes, mode) => (Mode::Exactly(*mode), Contents::Exactly(bytes)),
        Spec::Pam(_) | Spec::Dir(_) => (Mode::Protected, Contents::Opaque),
    };
    match (&artifact.spec, dir.file(name, mode, layout.owner, contents)) {
        (Spec::HostKey, _) if on_tmpfs(&dir) => Fact::Refused(format!(
            "{} is not persistent storage for the host key",
            parent.display()
        )),
        (Spec::Pam(render), Fact::Known(FileState::Absent)) => match vendor_pam(root, layout) {
            Fact::Known(FileState::Absent) if render.is_none() => Fact::Refused(format!(
                "{path} is absent and this host has no known PAM layout to render it from"
            )),
            Fact::Known(FileState::Absent) => Fact::Known(FileState::Absent),
            vendor => vendor,
        },
        (_, state) => state,
    }
}

/// A vendor PAM service in /usr/lib/pam.d also satisfies the broker.
fn vendor_pam(root: &Dir, layout: &Layout) -> Fact<FileState> {
    match root.walk(Path::new("usr/lib/pam.d"), layout.owner) {
        Fact::Known(Walk::Found(dir)) => {
            dir.file("hush", Mode::Protected, layout.owner, Contents::Opaque)
        }
        Fact::Known(Walk::Missing { .. }) => Fact::Known(FileState::Absent),
        Fact::Refused(why) => Fact::Refused(why),
        Fact::Unknown(why) => Fact::Unknown(why),
    }
}

fn pam_layout(root: &Dir, layout: &Layout) -> Option<linux::PamLayout> {
    let Fact::Known(Walk::Found(dir)) = root.walk(Path::new("etc/pam.d"), layout.owner) else {
        return None;
    };
    let ready = |name| {
        dir.file(name, Mode::Protected, layout.owner, Contents::Opaque)
            == Fact::Known(FileState::Ready)
    };
    if ready("common-auth") && ready("common-account") {
        Some(linux::PamLayout::Debian)
    } else {
        system_auth(root, layout).then_some(linux::PamLayout::SystemAuth)
    }
}

/// authselect owns system-auth as a link into etc/authselect. Only this
/// include may cross a native link, and its target keeps every file check.
fn system_auth(root: &Dir, layout: &Layout) -> bool {
    let logical = Path::new("/etc/pam.d/system-auth");
    let Fact::Known(target) = root.native_target(logical, layout.owner) else {
        return false;
    };
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return false;
    };
    let Fact::Known(Walk::Found(dir)) = root.walk(rel(parent), layout.owner) else {
        return false;
    };
    let name = name.to_string_lossy();
    dir.file(&name, Mode::Protected, layout.owner, Contents::Opaque)
        == Fact::Known(FileState::Ready)
}

#[cfg(target_os = "linux")]
fn on_tmpfs(dir: &Dir) -> bool {
    use std::os::fd::AsRawFd;
    // SAFETY: zeroed statfs is a valid out buffer for fstatfs on an open descriptor.
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    unsafe {
        libc::fstatfs(dir.fd().as_raw_fd(), &mut stat) == 0 && stat.f_type == libc::TMPFS_MAGIC
    }
}

#[cfg(not(target_os = "linux"))]
fn on_tmpfs(_: &Dir) -> bool {
    false
}

fn known<T: Clone>(fact: &Fact<T>) -> Option<T> {
    match fact {
        Fact::Known(value) => Some(value.clone()),
        _ => None,
    }
}
