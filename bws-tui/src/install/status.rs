//! Status: six independent facts. A failed query is unknown, never a false no.
//! Authentication is never checked, and no token is read.

use super::custody::FileState;
use super::facts::Fact;
use super::layout::Platform;
use super::observe::{Account, Observation, Spec};
use super::reach::{is_managed, Reach};
use std::fmt;

pub(crate) struct Status<'a>(pub &'a Observation);

impl fmt::Display for Status<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let o = self.0;
        writeln!(f, "installed: {}", installed(o))?;
        writeln!(f, "enabled: {}", enabled(o))?;
        writeln!(f, "active: {}", active(o))?;
        writeln!(f, "reachable: {}", reachable(o))?;
        writeln!(f, "credential: {}", credential(o))?;
        writeln!(
            f,
            "authentication: not checked. status never reads the token or contacts Bitwarden"
        )
    }
}

/// A refusal means no. Then an unknown means unknown. Then anything missing means no.
fn installed(o: &Observation) -> String {
    let mut problems: Vec<(bool, &str)> = o.checks.iter().filter_map(problem).collect();
    problems.extend(problem(&o.backend));
    problems.extend(problem(&o.bws));
    problems.extend(o.artifacts.iter().filter_map(|(_, fact)| problem(fact)));
    problems.extend(problem(&o.account));
    if let Fact::Refused(why) = &o.service {
        problems.push((true, why));
    }
    if let Some((_, why)) = problems.iter().find(|(refused, _)| *refused) {
        return format!("no ({why})");
    }
    if let Some((_, why)) = problems.first() {
        return format!("unknown ({why})");
    }
    let missing = o
        .artifacts
        .iter()
        .find(|(_, fact)| *fact != Fact::Known(FileState::Ready));
    match (missing, &o.account) {
        (Some((artifact, _)), _) => format!("no ({} is missing)", artifact.path.display()),
        (None, Fact::Known(Account::Absent(_))) => "no (account hushrun is missing)".into(),
        _ => "yes".into(),
    }
}

fn enabled(o: &Observation) -> String {
    let service_file = o.layout.service_file();
    let defined = o.artifacts.iter().any(|(artifact, fact)| {
        artifact.path.as_os_str() == service_file && *fact == Fact::Known(FileState::Ready)
    });
    describe(&o.service, |service| match o.layout.platform {
        Platform::Linux => yes_no(service.enabled),
        Platform::MacOs => yes_no(service.enabled && (service.loaded || defined)),
    })
}

fn active(o: &Observation) -> String {
    describe(&o.service, |service| match (service.active, service.pid) {
        (true, Some(pid)) => format!("yes (pid {pid})"),
        (true, None) => "yes".into(),
        (false, _) => "no".into(),
    })
}

fn reachable(o: &Observation) -> String {
    match (&o.socket, &o.service) {
        (Fact::Known(Reach::Nothing), _) => "no".into(),
        (Fact::Known(Reach::Peer(peer)), Fact::Known(service))
            if is_managed(service, peer, o.layout.owner) =>
        {
            format!(
                "yes (the managed daemon, pid {}, answers at {})",
                peer.pid,
                o.layout.socket.display()
            )
        }
        (Fact::Known(Reach::Peer(_)), Fact::Known(_)) => {
            "no (the listener is not the managed hush daemon)".into()
        }
        (Fact::Known(Reach::Peer(_)), _) => {
            "unknown (the listener cannot be matched to the managed daemon)".into()
        }
        (Fact::Unknown(why), _) => format!("unknown ({why})"),
        (Fact::Refused(why), _) => format!("no ({why})"),
    }
}

fn credential(o: &Observation) -> String {
    let Some((artifact, fact)) = o
        .artifacts
        .iter()
        .find(|(artifact, _)| artifact.spec == Spec::Credential)
    else {
        return match problem(&o.backend) {
            Some((true, why)) => format!("refused ({why})"),
            Some((false, why)) => format!("unknown ({why})"),
            None => "unknown".into(),
        };
    };
    let path = artifact.path.display();
    match fact {
        Fact::Known(FileState::Ready) => {
            format!("metadata checked at {path}, owner-only 0600, contents not read")
        }
        Fact::Known(FileState::Resume { .. }) => {
            format!("metadata checked at {path}, a leftover staging link remains")
        }
        Fact::Known(FileState::Absent) => format!("absent ({path})"),
        Fact::Refused(why) => format!("refused ({why})"),
        Fact::Unknown(why) => format!("unknown ({why})"),
    }
}

fn describe<T>(fact: &Fact<T>, known: impl FnOnce(&T) -> String) -> String {
    match fact {
        Fact::Known(value) => known(value),
        Fact::Refused(why) => format!("no ({why})"),
        Fact::Unknown(why) => format!("unknown ({why})"),
    }
}

fn problem<T>(fact: &Fact<T>) -> Option<(bool, &str)> {
    match fact {
        Fact::Known(_) => None,
        Fact::Refused(why) => Some((true, why)),
        Fact::Unknown(why) => Some((false, why)),
    }
}

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.into()
}
