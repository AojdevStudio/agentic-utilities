//! Human operations from the client. The client sends metadata and passes
//! descriptors. It never sees a value or a password: the daemon reads input
//! from the passed descriptors and writes disclosures to the passed terminal.

use super::approve::APPROVAL_TIMEOUT;
use super::backend::BWS_TIMEOUT;
use super::client::Client;
use super::meta::ProjectMeta;
use super::operation::{Operation, Outcome, Refusal};
use super::peercred::read_peer_credentials;
use super::protocol::{
    read_frame, write_frame, write_frame_with_rights, Frame, RequestFrame, ResponseFrame,
};
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::os::fd::BorrowedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::time::Duration;

/// One operation can wait on an approval prompt, a value prompt, a reveal
/// screen, and several vault calls. The client never gives up earlier.
pub(crate) const HUMAN_TIMEOUT: Duration =
    Duration::from_secs(3 * APPROVAL_TIMEOUT.as_secs() + 4 * BWS_TIMEOUT.as_secs());

impl Client {
    pub(crate) fn projects(&mut self) -> Result<Vec<ProjectMeta>> {
        write_frame(self.stream_mut(), &Frame::Request(RequestFrame::Projects))?;
        match read_frame(self.stream_mut())? {
            Frame::Response(ResponseFrame::Projects { projects }) => Ok(projects),
            Frame::Response(ResponseFrame::Refused { refusal }) => bail!("{refusal}"),
            Frame::Response(ResponseFrame::Error { message, .. }) => bail!("{message}"),
            _ => bail!("unexpected frame from daemon"),
        }
    }

    /// Send one operation with its descriptors, in the order its contract
    /// names. A refusal is a normal answer, not a transport error.
    pub(crate) fn operate(
        &mut self,
        op: Operation,
        fds: &[BorrowedFd<'_>],
    ) -> Result<Result<Outcome, Refusal>> {
        if !fds.is_empty() {
            self.verify_daemon()?;
        }
        let stream = self.stream_mut();
        stream.set_read_timeout(Some(HUMAN_TIMEOUT))?;
        write_frame_with_rights(stream, &Frame::Request(RequestFrame::Human { op }), fds)?;
        match read_frame(stream)? {
            Frame::Response(ResponseFrame::Done { outcome }) => Ok(Ok(outcome)),
            Frame::Response(ResponseFrame::Refused { refusal }) => Ok(Err(refusal)),
            Frame::Response(ResponseFrame::Error { message, .. }) => bail!("{message}"),
            _ => bail!("unexpected frame from daemon"),
        }
    }

    /// Descriptors go only to a root daemon or to this user's own development daemon.
    fn verify_daemon(&mut self) -> Result<()> {
        let daemon = read_peer_credentials(self.stream_mut())?;
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        if daemon.uid != 0 && daemon.uid != euid {
            bail!(
                "refusing to pass a terminal to a daemon owned by uid {}",
                daemon.uid
            );
        }
        Ok(())
    }
}

/// Open the real terminal device behind stdin, stdout, or stderr for the
/// daemon to use. `/dev/tty` is not passed: its descriptor reports the
/// `/dev/tty` node, not the user's own terminal, and the daemon refuses it.
pub(crate) fn open_terminal() -> Result<File> {
    for fd in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        let mut name = [0 as libc::c_char; 256];
        // SAFETY: ttyname_r writes a NUL-terminated path into the buffer.
        if unsafe { libc::ttyname_r(fd, name.as_mut_ptr(), name.len()) } != 0 {
            continue;
        }
        // SAFETY: on success the buffer holds a NUL-terminated path.
        let path = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) };
        let path = std::path::Path::new(std::ffi::OsStr::from_bytes(path.to_bytes()));
        return std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_CLOEXEC)
            .open(path)
            .with_context(|| format!("failed to open the terminal at {}", path.display()));
    }
    bail!("this command needs an interactive terminal on stdin, stdout, or stderr")
}
