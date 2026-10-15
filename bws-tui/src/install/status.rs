//! Independent status facts from the shared observation. Authentication is
//! never checked: status reads no token and never contacts BWS.

use super::layout::Kind;
use super::model::{Fact, Presence, Socket};
use super::observe::Observed;
use std::fmt::Write;

/// A three-way answer with a fixed reason for anything but yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Yes(String),
    No(&'static str),
    Unknown(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub installed: Answer,
    pub enabled: Answer,
    pub active: Answer,
    pub reachable: Answer,
    pub credential: Answer,
}

fn gap<T>(fact: &Fact<T>) -> Option<Answer> {
    match fact {
        Fact::Known(_) => None,
        Fact::Refused(reason) => Some(Answer::No(reason)),
        Fact::Unknown(reason) => Some(Answer::Unknown(reason)),
    }
}

impl Status {
    pub(crate) fn from_observed(observed: &Observed) -> Self {
        let presences = observed.entries.iter().map(|entry| &entry.presence);
        let installed = [
            gap(&observed.hush),
            gap(&observed.bws),
            gap(&observed.backend),
            gap(&observed.rendered),
            gap(&observed.host_key),
            gap(&observed.account),
        ]
        .into_iter()
        .chain(presences.map(gap))
        .flatten()
        .min_by_key(|answer| matches!(answer, Answer::No(_)))
        .unwrap_or_else(|| {
            let missing = observed.entries.iter().any(|e| e.presence == Fact::Known(Presence::Absent))
                || observed.account == Fact::Known(Presence::Absent)
                || observed.host_key == Fact::Known(Presence::Absent);
            if missing {
                Answer::No("some installed files or the hushrun account are missing")
            } else {
                Answer::Yes(String::new())
            }
        });
        let manager = &observed.manager;
        let enabled = gap(manager).unwrap_or_else(|| match manager.known() {
            Some(m) if m.enabled && (m.loaded || cfg!(target_os = "macos")) => Answer::Yes(String::new()),
            _ => Answer::No("boot policy does not start the service"),
        });
        let active = gap(manager).unwrap_or_else(|| match manager.known().and_then(|m| m.pid) {
            Some(pid) => Answer::Yes(format!("pid {pid}")),
            None => Answer::No("the manager reports no running service"),
        });
        let reachable = gap(&observed.socket).unwrap_or_else(|| match observed.socket.known() {
            Some(Socket::Managed) => Answer::Yes("root, matching the service pid".into()),
            Some(Socket::Foreign) => Answer::No("the peer is not the managed service"),
            _ => Answer::No("nothing listens on the broker socket"),
        });
        let token = observed.entries.iter().find(|e| matches!(e.spec.kind, Kind::Token(_)));
        let credential = match token.map(|e| &e.presence) {
            None => gap(&observed.backend).unwrap_or(Answer::Unknown("no token backend")),
            Some(Fact::Known(Presence::Absent)) => Answer::No("the token source is missing"),
            Some(Fact::Known(_)) => Answer::Yes("owner, mode, type, and parents checked".into()),
            Some(fact) => gap(fact).unwrap_or(Answer::Unknown("not observed")),
        };
        Self { installed, enabled, active, reachable, credential }
    }

    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        for (name, answer) in [
            ("installed", &self.installed),
            ("enabled", &self.enabled),
            ("active", &self.active),
            ("reachable", &self.reachable),
            ("credential metadata", &self.credential),
        ] {
            let _ = match answer {
                Answer::Yes(detail) if detail.is_empty() => writeln!(out, "{name}: yes"),
                Answer::Yes(detail) => writeln!(out, "{name}: yes ({detail})"),
                Answer::No(reason) => writeln!(out, "{name}: no ({reason})"),
                Answer::Unknown(reason) => writeln!(out, "{name}: unknown ({reason})"),
            };
        }
        out.push_str("authentication: not checked (status never contacts BWS)\n");
        out
    }
}
