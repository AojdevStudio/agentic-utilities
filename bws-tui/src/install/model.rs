//! Installer domain facts. A fact is known, refused, or unknown. A reason is
//! fixed text, never a value read from the host, so no report can echo one.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fact<T> {
    Known(T),
    /// Observed, and unsafe or unsupported for this install.
    Refused(&'static str),
    /// Could not be observed, for example without root.
    Unknown(&'static str),
}

impl<T> Fact<T> {
    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> Fact<U> {
        self.and_then(|value| Fact::Known(f(value)))
    }

    pub(crate) fn and_then<U>(self, f: impl FnOnce(T) -> Fact<U>) -> Fact<U> {
        match self {
            Fact::Known(value) => f(value),
            Fact::Refused(reason) => Fact::Refused(reason),
            Fact::Unknown(reason) => Fact::Unknown(reason),
        }
    }

    pub(crate) fn known(&self) -> Option<&T> {
        match self {
            Fact::Known(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_ref(&self) -> Fact<&T> {
        match self {
            Fact::Known(value) => Fact::Known(value),
            Fact::Refused(reason) => Fact::Refused(reason),
            Fact::Unknown(reason) => Fact::Unknown(reason),
        }
    }
}

/// Where the service reads its token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Backend {
    /// systemd 250 or newer decrypts a host-key credential at start.
    Encrypted,
    /// systemd 247 through 249 copies a root-only file into the credential.
    LoadCredential,
    /// macOS reads the root-only file directly.
    File,
}

impl Backend {
    /// LoadCredential first shipped in systemd 247, encrypted credentials in 250.
    pub(crate) fn for_systemd(version: u32) -> Fact<Self> {
        match version {
            250.. => Fact::Known(Self::Encrypted),
            247..=249 => Fact::Known(Self::LoadCredential),
            _ => Fact::Refused("systemd older than 247 cannot pass a credential to the service"),
        }
    }
}

/// What exists at one managed path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    Absent,
    /// Present with the exact expected type, owner, mode, and content.
    Ready,
    /// A safe service the operator already manages. Install keeps it.
    Operator,
}

/// The service manager's view of the hush service. Linux has no boot
/// policy for an unloaded unit. macOS keeps its disabled override apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Manager {
    pub loaded: bool,
    pub enabled: bool,
    pub pid: Option<u32>,
    /// The definition changed on disk after the manager loaded it.
    pub stale: bool,
}

/// Who answers on the broker socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Socket {
    Absent,
    /// A socket file that nothing listens on. The daemon replaces it.
    Stale,
    /// Root, with the manager's own service pid, on the same connection.
    Managed,
    Foreign,
}

/// One account the installer creates. Native tools pick the Linux uid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRequest {
    pub uid: Option<u32>,
    pub gid: u32,
    pub home: PathBuf,
    pub shell: PathBuf,
}

/// The run account as the native account database reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeAccount {
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
    pub shell: PathBuf,
    pub login_disabled: bool,
    /// No other account shares this uid or this name.
    pub unique: bool,
}

/// Exact readback rule for an existing run account. Its uid was chosen by
/// whoever created it, so only root and uniqueness are checked.
pub(crate) fn judge_account(found: &NativeAccount, want: &AccountRequest) -> Fact<Presence> {
    let matches = found.uid != 0
        && found.gid == want.gid
        && found.home == want.home
        && found.shell == want.shell
        && found.login_disabled
        && found.unique;
    if matches {
        Fact::Known(Presence::Ready)
    } else {
        Fact::Refused("the hushrun account exists with settings install did not choose")
    }
}
