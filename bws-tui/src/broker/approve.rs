//! Fresh password approval for one request. The account comes from the
//! kernel peer uid, never from the client. The password lives only in a
//! bounded zeroizing buffer and is never cached.

use super::console::{escape, Console, Interrupt};
use super::identity::{AccountDb, SystemAccounts};
use super::operation::Refusal;
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// How long the daemon waits for a human at one prompt.
pub(crate) const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// Longest password the daemon reads.
pub(crate) const MAX_PASSWORD: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Approved,
    Denied,
}

/// The operating system's account password check.
pub(crate) trait Authenticator: Send + Sync {
    /// Refuses before any prompt when the check cannot run here. The reason is shown to the user.
    fn available(&self) -> Result<(), String>;
    /// A wrong password is `Ok(Denied)`. `Err` means the check could not run.
    fn verify(&self, account: &str, password: &[u8]) -> Result<Verdict, String>;
}

/// Proof that one request was approved just now. Only [`approve`] makes one.
pub(crate) struct Approved(());

/// What the human approves. Shown before the password prompt.
pub(crate) struct ApprovalRequest<'a> {
    pub verb: &'a str,
    pub key: &'a str,
    pub project: Option<&'a str>,
    pub id: &'a str,
    pub uid: u32,
    pub pid: i32,
}

/// Ask for the peer account's password on the checked terminal and verify it.
pub(crate) fn approve(
    console: &mut Console,
    request: &ApprovalRequest<'_>,
    auth: &dyn Authenticator,
    socket: &UnixStream,
) -> Result<Approved, Refusal> {
    auth.available()
        .map_err(|reason| Refusal::ApprovalUnavailable { reason })?;
    let account = match SystemAccounts.by_uid(request.uid) {
        Ok(Some(account)) => account.name,
        _ => {
            return Err(Refusal::ApprovalUnavailable {
                reason: format!("uid {} has no account name", request.uid),
            })
        }
    };
    let banner = format!(
        "\r\nhush: approve {} of {} in {} ({}) for process {}\r\n",
        request.verb,
        escape(request.key.as_bytes()),
        escape(request.project.unwrap_or("no project").as_bytes()),
        escape(request.id.as_bytes()),
        request.pid,
    );
    console.say(&banner).map_err(|_| Refusal::TerminalFailed)?;
    let prompt = format!("Password for {}: ", escape(account.as_bytes()));
    let password = console
        .read_hidden(&prompt, MAX_PASSWORD, socket)
        .map_err(|interrupt| match interrupt {
            Interrupt::TooLong => Refusal::ApprovalDenied,
            _ => Refusal::ApprovalCancelled,
        })?;
    match auth.verify(&account, &password) {
        Ok(Verdict::Approved) => Ok(Approved(())),
        Ok(Verdict::Denied) => {
            let _ = console.say("hush: approval denied\r\n");
            Err(Refusal::ApprovalDenied)
        }
        Err(reason) => Err(Refusal::ApprovalUnavailable { reason }),
    }
}

/// Refuses everywhere. Used where no native password check exists.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
struct Unsupported;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl Authenticator for Unsupported {
    fn available(&self) -> Result<(), String> {
        Err("no password approval exists for this platform".into())
    }

    fn verify(&self, _account: &str, _password: &[u8]) -> Result<Verdict, String> {
        Err("no password approval exists for this platform".into())
    }
}

/// The native account password check for this platform.
pub(crate) fn platform_authenticator() -> std::sync::Arc<dyn Authenticator> {
    #[cfg(target_os = "linux")]
    return std::sync::Arc::new(super::pam::PamAuthenticator::system());
    #[cfg(target_os = "macos")]
    return std::sync::Arc::new(super::opendirectory::OpenDirectoryAuthenticator);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return std::sync::Arc::new(Unsupported);
}
