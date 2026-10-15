//! One read-only observation shared by install and status. It never reads a
//! token, never prompts, and never contacts BWS. Without root, private
//! objects come back unknown instead of absent.

use super::files::{check_executable, inspect, open_node, open_parent, Stop};
use super::host::Host;
use super::layout::{definition_path, host_key_path, private_specs, socket_path, Kind, Layout, Spec};
use super::model::{judge_account, AccountRequest, Backend, Fact, Manager, Presence, Socket};
use super::probe;
use super::render::{self, Service};
use crate::broker::paths::DEFAULT_BWS_CANDIDATES;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// The executables the service will run.
pub(crate) struct Binaries {
    pub hush: PathBuf,
    /// Discovered from the trusted candidates when absent.
    pub bws: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub spec: Spec,
    pub presence: Fact<Presence>,
}

#[derive(Debug, Clone)]
pub(crate) struct Observed {
    pub hush: Fact<PathBuf>,
    pub bws: Fact<PathBuf>,
    pub backend: Fact<Backend>,
    /// Every managed artifact that could be described, in creation order.
    pub entries: Vec<Entry>,
    /// Why the PAM service or the service definition could not be rendered.
    pub rendered: Fact<()>,
    pub host_key: Fact<Presence>,
    pub request: Fact<AccountRequest>,
    pub account: Fact<Presence>,
    pub manager: Fact<Manager>,
    pub socket: Fact<Socket>,
}

pub(crate) fn observe(layout: &Layout, binaries: &Binaries, host: &mut dyn Host) -> Observed {
    let hush = executable(layout, &binaries.hush);
    let bws = match &binaries.bws {
        Some(path) => executable(layout, path),
        None => discover_bws(layout),
    };
    let backend = if cfg!(target_os = "macos") {
        Fact::Known(Backend::File)
    } else {
        host.systemd_version().and_then(Backend::for_systemd)
    };
    let mut entries: Vec<Entry> = Vec::new();
    if let Fact::Known(backend) = backend {
        for spec in private_specs(backend) {
            push(layout, &mut entries, spec);
        }
    }
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut rendered = Fact::Known(());
    #[cfg(target_os = "linux")]
    match pam_stack(layout) {
        Fact::Known(stack) => push(layout, &mut entries, Spec {
            path: PathBuf::from("/etc/pam.d").join(crate::broker::pam::SERVICE),
            kind: Kind::Pam(render::pam(stack)),
        }),
        other => rendered = other.map(|_| ()),
    }
    let definition = match (&hush, &bws, &backend) {
        (Fact::Known(hush), Fact::Known(bws), Fact::Known(backend)) => {
            render::definition(&Service { hush, bws, backend: *backend })
        }
        (hush, bws, backend) => first_gap(&[hush.as_ref().map(|_| ()), bws.as_ref().map(|_| ()), backend.as_ref().map(|_| ())]),
    };
    match definition {
        Fact::Known(bytes) => push(layout, &mut entries, Spec { path: definition_path(), kind: Kind::File(bytes) }),
        other => rendered = rendered.and_then(|()| other.map(|_| ())),
    }
    let host_key = match backend {
        Fact::Known(Backend::Encrypted) => inspect_host_key(layout),
        Fact::Known(_) => Fact::Known(Presence::Ready),
        Fact::Refused(reason) => Fact::Refused(reason),
        Fact::Unknown(reason) => Fact::Unknown(reason),
    };
    let request = account_request(layout, host);
    let account = host.account().and_then(|found| match found {
        None => Fact::Known(Presence::Absent),
        Some(found) => request.as_ref().and_then(|want| judge_account(&found, want)),
    });
    let manager = host.manager();
    let socket = probe::judge(probe::connect(&layout.display(&socket_path())), layout.owner, &manager);
    Observed { hush, bws, backend, entries, rendered, host_key, request, account, manager, socket }
}

fn first_gap(facts: &[Fact<()>]) -> Fact<Vec<u8>> {
    facts
        .iter()
        .find_map(|fact| match fact {
            Fact::Known(()) => None,
            Fact::Refused(reason) => Some(Fact::Refused(reason)),
            Fact::Unknown(reason) => Some(Fact::Unknown(reason)),
        })
        .unwrap_or(Fact::Unknown("the service definition could not be rendered"))
}

fn push(layout: &Layout, entries: &mut Vec<Entry>, spec: Spec) {
    let parent_managed = entries
        .iter()
        .any(|entry| entry.spec.kind == Kind::Dir && Some(entry.spec.path.as_path()) == spec.path.parent());
    let presence = inspect(layout, &spec, parent_managed);
    entries.push(Entry { spec, presence });
}

/// An executable path as the service will see it, checked from the anchor.
fn executable(layout: &Layout, real: &Path) -> Fact<PathBuf> {
    let Ok(rest) = real.strip_prefix(&layout.anchor) else {
        return Fact::Refused("the executable is outside the protected tree");
    };
    let logical = Path::new("/").join(rest);
    check_executable(layout, &logical).map(|()| logical)
}

fn discover_bws(layout: &Layout) -> Fact<PathBuf> {
    let mut unknown = None;
    for candidate in DEFAULT_BWS_CANDIDATES {
        match check_executable(layout, Path::new(candidate)) {
            Fact::Known(()) => return Fact::Known(PathBuf::from(candidate)),
            Fact::Unknown(reason) => unknown = unknown.or(Some(reason)),
            Fact::Refused(_) => {}
        }
    }
    match unknown {
        Some(reason) => Fact::Unknown(reason),
        None => Fact::Refused("no trusted bws executable at /usr/local/bin/bws or /usr/bin/bws"),
    }
}

/// A root-owned regular file nobody else can change, such as a PAM include.
#[cfg(target_os = "linux")]
fn trusted_file(layout: &Layout, logical: &str) -> Fact<bool> {
    match open_node(layout, Path::new(logical), false) {
        Ok(node) => {
            let mode = node.stat.st_mode as u32;
            let regular = mode & libc::S_IFMT as u32 == libc::S_IFREG as u32;
            let safe = regular && node.stat.st_uid == layout.owner && mode & 0o022 == 0;
            if safe && super::acl::grants_access(node.fd.as_raw_fd()) == Ok(false) {
                Fact::Known(true)
            } else {
                Fact::Refused("a PAM include is not a protected root file")
            }
        }
        Err(Stop::Missing) => Fact::Known(false),
        Err(stop) => stop.fact(),
    }
}

#[cfg(target_os = "linux")]
fn pam_stack(layout: &Layout) -> Fact<render::PamStack> {
    let common = trusted_file(layout, "/etc/pam.d/common-auth")
        .and_then(|auth| trusted_file(layout, "/etc/pam.d/common-account").map(|account| auth && account));
    match common {
        Fact::Known(true) => Fact::Known(render::PamStack::Common),
        Fact::Known(false) => trusted_file(layout, "/etc/pam.d/system-auth").and_then(|found| {
            if found {
                Fact::Known(render::PamStack::SystemAuth)
            } else {
                Fact::Refused("this distro's PAM layout is not one install knows")
            }
        }),
        other => other.map(|_| render::PamStack::Common),
    }
}

/// The systemd host key must sit on persistent storage, or the encrypted
/// token would become unreadable after a reboot.
fn inspect_host_key(layout: &Layout) -> Fact<Presence> {
    let path = host_key_path();
    #[cfg(target_os = "linux")]
    match open_parent(layout, &path) {
        Ok((dir, _)) if super::files::is_volatile(dir.as_raw_fd()) != Some(false) => {
            return Fact::Refused("the systemd credential directory is not persistent storage");
        }
        Ok(_) => {}
        Err(stop) => return stop.fact(),
    }
    #[cfg(not(target_os = "linux"))]
    let _ = open_parent;
    let spec = Spec { path, kind: Kind::Token(Backend::Encrypted) };
    match open_node(layout, &spec.path, false) {
        Err(Stop::Missing) => Fact::Known(Presence::Absent),
        Err(stop) => stop.fact(),
        Ok(node) => {
            let mode = node.stat.st_mode as u32;
            let safe = node.stat.st_uid == layout.owner
                && mode & libc::S_IFMT as u32 == libc::S_IFREG as u32
                && mode & 0o7777 == 0o400
                && node.stat.st_nlink == 1
                && super::acl::grants_access(node.fd.as_raw_fd()) == Ok(false);
            if safe {
                Fact::Known(Presence::Ready)
            } else {
                Fact::Refused("the systemd host key is not a root-only 0400 file")
            }
        }
    }
}

/// The account install would create: nobody's group, a nonlogin shell, and a
/// home no human uses. macOS also needs a free system uid chosen up front.
fn account_request(layout: &Layout, host: &mut dyn Host) -> Fact<AccountRequest> {
    let (home, shells): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("/var/empty", &["/usr/bin/false"])
    } else {
        ("/nonexistent", &["/usr/sbin/nologin", "/sbin/nologin"])
    };
    let shell = shells
        .iter()
        .find(|shell| check_executable(layout, Path::new(shell)) == Fact::Known(()))
        .map_or(Fact::Refused("no protected nonlogin shell exists"), |shell| Fact::Known(PathBuf::from(shell)));
    let gid = host.nobody_gid().and_then(|gid| {
        if gid == 0 {
            Fact::Refused("the nobody account's group is root")
        } else {
            Fact::Known(gid)
        }
    });
    let uid = if cfg!(target_os = "macos") {
        host.used_uids().and_then(|used| {
            (200..500)
                .find(|uid| !used.contains(uid))
                .map_or(Fact::Refused("no free system uid between 200 and 499"), |uid| Fact::Known(Some(uid)))
        })
    } else {
        Fact::Known(None)
    };
    shell.and_then(|shell| gid.and_then(|gid| uid.map(|uid| AccountRequest { uid, gid, home: PathBuf::from(home), shell })))
}
