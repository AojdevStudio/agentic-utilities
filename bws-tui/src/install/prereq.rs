//! Prerequisites with no artifact of their own: policy, macOS aliases, the
//! trusted bws binary and the service manager's view of hush.

use super::custody::{Contents, Dir, FileState, Mode, Walk};
use super::facts::{io_fact, Fact, Service};
use super::host::{Answer, Host, Query};
use super::layout::{rel, Backend, Layout, Platform};
use super::{linux, macos};
use crate::broker::policy::Policy;
use std::io;
use std::path::{Path, PathBuf};

/// Prerequisites checked at observation and again before each manager action.
pub(crate) fn checks(layout: &Layout, root: &Dir) -> Vec<Fact<()>> {
    let mut checks = vec![
        root.executable(rel(&layout.executable), layout.owner),
        policy(layout, root),
    ];
    if layout.platform == Platform::MacOs {
        checks.push(aliases(layout));
    }
    checks
}

/// A manager action may proceed when it remains needed. Linux can also
/// discover the just-published unit before daemon-reload.
pub(crate) fn action_needed(
    observed: &Fact<Service>,
    current: &Fact<Service>,
    needed: fn(&Service) -> bool,
    allow_discovered_unit: bool,
) -> bool {
    matches!(current, Fact::Known(service) if needed(service))
        || (allow_discovered_unit
            && matches!(observed, Fact::Known(service) if !service.loaded)
            && matches!(current, Fact::Known(service) if service.loaded && !service.needs_reload))
}

/// An existing policy must be protected and valid, below protected ancestors,
/// with no ACL grant on any of them. Absence is the empty policy.
pub(crate) fn policy(layout: &Layout, root: &Dir) -> Fact<()> {
    let logical = layout.policy();
    let (parent, name) = logical.rsplit_once('/').unwrap_or(("", logical));
    let dir = match root.walk(rel(Path::new(parent)), layout.owner) {
        Fact::Known(Walk::Found(dir)) => dir,
        Fact::Known(Walk::Missing { .. }) => return Fact::Known(()),
        Fact::Refused(why) => return Fact::Refused(why),
        Fact::Unknown(why) => return Fact::Unknown(why),
    };
    match dir.file(name, Mode::Protected, layout.owner, Contents::Opaque) {
        Fact::Known(FileState::Absent) => return Fact::Known(()),
        Fact::Known(FileState::Ready) => {}
        Fact::Known(FileState::Resume { .. }) => {
            return Fact::Refused(format!("{logical} has another hard link"))
        }
        Fact::Refused(why) => return Fact::Refused(why),
        Fact::Unknown(why) => return Fact::Unknown(why),
    }
    // The checked ancestors let only root replace the file the parser opens.
    let dev_owner = layout.owner.0 != 0;
    match Policy::load(&layout.host_path(Path::new(logical)), dev_owner) {
        Ok(_) => Fact::Known(()),
        Err(error)
            if error
                .chain()
                .filter_map(|cause| cause.downcast_ref::<io::Error>())
                .any(|io| io.kind() == io::ErrorKind::PermissionDenied) =>
        {
            Fact::Unknown(format!("{logical} is not readable by this user"))
        }
        Err(error) => Fact::Refused(format!(
            "{logical} is not a protected valid policy ({error:#})"
        )),
    }
}

/// macOS keeps /etc and /var as root-owned links into /private.
pub(crate) fn aliases(layout: &Layout) -> Fact<()> {
    for (alias, target) in [("var", "private/var"), ("etc", "private/etc")] {
        let path = layout.root.join(alias);
        let native = std::fs::symlink_metadata(&path).is_ok_and(|metadata| {
            use std::os::unix::fs::MetadataExt;
            metadata.file_type().is_symlink()
                && (metadata.uid() == 0 || metadata.uid() == layout.owner.0)
        }) && std::fs::read_link(&path).is_ok_and(|link| link == Path::new(target));
        if !native {
            return Fact::Refused(format!("/{alias} is not the native link to /{target}"));
        }
    }
    Fact::Known(())
}

/// The first bws candidate whose canonical path passes executable custody.
pub(crate) fn resolve_bws(layout: &Layout, root: &Dir) -> Fact<PathBuf> {
    // Compare canonical with canonical. A root below a native alias, such as
    // macOS /var, would never prefix its own canonical children otherwise.
    let base = match layout.root.canonicalize() {
        Ok(base) => base,
        Err(error) => return io_fact(&layout.root, &error),
    };
    let mut first_problem = None;
    for candidate in &layout.bws_candidates {
        let canonical = match layout.host_path(candidate).canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                first_problem.get_or_insert(Fact::Unknown(format!(
                    "{} could not be resolved ({error})",
                    candidate.display()
                )));
                continue;
            }
        };
        let Ok(inside) = canonical.strip_prefix(&base) else {
            first_problem.get_or_insert(Fact::Refused(format!(
                "{} resolves outside the install root",
                candidate.display()
            )));
            continue;
        };
        let logical = Path::new("/").join(inside);
        match root.executable(rel(&logical), layout.owner) {
            Fact::Known(()) => return Fact::Known(logical),
            Fact::Refused(why) => drop(first_problem.get_or_insert(Fact::Refused(why))),
            Fact::Unknown(why) => drop(first_problem.get_or_insert(Fact::Unknown(why))),
        }
    }
    let names: Vec<String> = layout
        .bws_candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    first_problem.unwrap_or_else(|| {
        Fact::Refused(format!(
            "no trusted bws binary at {}. hush install never provisions bws",
            names.join(" or ")
        ))
    })
}

/// The manager's view. A failed query is Unknown, never disabled or inactive.
/// A loaded definition must run the rendered command as its rendered identity,
/// with no command in a list the rendered unit leaves empty.
/// On macOS `enabled` means no disabled override for label hush.
pub(crate) fn service(
    layout: &Layout,
    host: &mut dyn Host,
    backend: Option<Backend>,
    bws: Option<&Path>,
) -> Fact<Service> {
    if layout.platform == Platform::Linux {
        let service = match host.query(Query::UnitShow) {
            Answer::Text(text) => linux::parse_unit_show(&text, layout.service_file()),
            Answer::NotFound => Fact::Unknown("systemctl show gave no reply".into()),
            Answer::Failed(why) => Fact::Unknown(why),
        };
        let Fact::Known(found) = service else {
            return service;
        };
        if !found.loaded {
            return service;
        }
        let (Some(backend), Some(bws)) = (backend, bws) else {
            return Fact::Unknown(
                "the loaded unit cannot be checked without a known backend and a trusted bws"
                    .into(),
            );
        };
        let args = match layout.daemon_args(backend, bws) {
            Ok(args) => args,
            Err(why) => return Fact::Refused(why),
        };
        for list in std::iter::once("ExecStart").chain(linux::UNRENDERED_COMMANDS) {
            let check = match host.query(Query::UnitCommands(list)) {
                Answer::Text(json) if list == "ExecStart" => linux::loaded_command(&json, &args),
                Answer::Text(json) => linux::no_command(&json, list),
                Answer::NotFound => Fact::Unknown("busctl gave no reply".into()),
                Answer::Failed(why) => Fact::Unknown(why),
            };
            match check {
                Fact::Known(()) => {}
                Fact::Refused(why) => return Fact::Refused(why),
                Fact::Unknown(why) => return Fact::Unknown(why),
            }
        }
        return service;
    }
    let mut service = match host.query(Query::LaunchdPrint) {
        Answer::NotFound => Service::default(),
        Answer::Failed(why) => return Fact::Unknown(why),
        Answer::Text(text) => {
            let Some(bws) = bws else {
                return Fact::Unknown(
                    "the loaded launchd job cannot be checked without a trusted bws".into(),
                );
            };
            let args = match layout.daemon_args(Backend::Launchd, bws) {
                Ok(args) => args,
                Err(why) => return Fact::Refused(why),
            };
            match macos::parse_print(&text, layout.service_file(), &args) {
                Fact::Known(service) => service,
                other => return other,
            }
        }
    };
    service.enabled = match host.query(Query::LaunchdDisabled) {
        Answer::Text(text) => macos::disabled_override(&text) != Some(true),
        Answer::NotFound => true,
        Answer::Failed(why) => return Fact::Unknown(why),
    };
    Fact::Known(service)
}
