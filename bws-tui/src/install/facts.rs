//! Observed domain facts shared by status, the planner and the platform parsers.

use std::io;
use std::path::Path;

/// One observed fact. Install refuses anything but `Known`. Status prints all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fact<T> {
    Known(T),
    /// The fact is known and conflicts with the install contract.
    Refused(String),
    /// The fact could not be observed, for example without root.
    Unknown(String),
}

/// The service manager's view of the hush service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Service {
    pub loaded: bool,
    pub needs_reload: bool,
    pub enabled: bool,
    pub active: bool,
    pub pid: Option<i32>,
}

/// One native account record, normalized across platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRecord {
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub locked: bool,
}

pub(crate) fn refused<T>(path: &Path, why: String) -> Fact<T> {
    Fact::Refused(format!("{} {why}", path.display()))
}

/// Permission errors become Unknown. A symlink or non-directory where a
/// directory belongs is a refusal. Anything else stays Unknown with its cause.
pub(crate) fn io_fact<T>(path: &Path, error: &io::Error) -> Fact<T> {
    match error.raw_os_error() {
        Some(libc::EACCES | libc::EPERM) => {
            Fact::Unknown(format!("{} is not readable by this user", path.display()))
        }
        Some(libc::ELOOP | libc::ENOTDIR) => {
            refused(path, "is a symbolic link or not a directory".into())
        }
        _ => Fact::Unknown(format!(
            "{} could not be inspected ({error})",
            path.display()
        )),
    }
}
