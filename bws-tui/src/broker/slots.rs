//! Binds the passed descriptors to the roles an operation names and checks
//! each one before the daemon uses it.

use super::approve::APPROVAL_TIMEOUT;
use super::console::{Console, Interrupt};
use super::fdpass::MAX_RIGHTS;
use super::material::SecretMaterial;
use super::mutate::{invalid, material, read_input, MAX_VALUE};
use super::operation::{DescriptorProblem, FdRole, Refusal, ValueSource};
use super::peercred::PeerCredentials;
use super::tty::{check_terminal, check_value_input};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::time::Instant;

/// The passed descriptors, checked against the roles the operation names.
/// Activity on the client's socket cancels any wait on them.
pub(crate) struct Slots<'a> {
    socket: &'a UnixStream,
    console: Option<Console<'a>>,
    value: Option<OwnedFd>,
}

impl<'a> Slots<'a> {
    pub(crate) fn bind(
        roles: &[FdRole],
        fds: Vec<OwnedFd>,
        peer: PeerCredentials,
        socket: &'a UnixStream,
    ) -> Result<Self, Refusal> {
        debug_assert!(
            roles.len() <= MAX_RIGHTS,
            "the frame limit admits every role"
        );
        if fds.len() != roles.len() {
            return Err(descriptors(DescriptorProblem::WrongCount {
                expected: roles.len() as u8,
                received: fds.len().min(u8::MAX as usize) as u8,
            }));
        }
        let mut slots = Self {
            socket,
            console: None,
            value: None,
        };
        for (role, fd) in roles.iter().zip(fds) {
            match role {
                FdRole::Terminal => {
                    check_terminal(fd.as_fd(), peer).map_err(descriptors)?;
                    let console = Console::new(fd, socket).map_err(|_| Refusal::TerminalFailed)?;
                    slots.console = Some(console);
                }
                FdRole::ValueInput => {
                    check_value_input(fd.as_fd()).map_err(descriptors)?;
                    slots.value = Some(fd);
                }
            }
        }
        Ok(slots)
    }

    pub(crate) fn console(&mut self) -> Result<&mut Console<'a>, Refusal> {
        self.console
            .as_mut()
            .ok_or(descriptors(DescriptorProblem::Malformed))
    }

    /// The new value, from the passed input or a hidden prompt.
    pub(crate) fn read_value(
        &mut self,
        source: ValueSource,
        prompt: &str,
    ) -> Result<SecretMaterial, Refusal> {
        match source {
            ValueSource::InputFd => match self.value.take() {
                Some(fd) => read_input(fd, self.socket, Instant::now() + APPROVAL_TIMEOUT),
                None => Err(descriptors(DescriptorProblem::Malformed)),
            },
            ValueSource::TtyPrompt => {
                let line = self.console()?.read_hidden(prompt, MAX_VALUE);
                material(line.map_err(|interrupt| match interrupt {
                    Interrupt::TooLong => invalid("the value is larger than 64 KiB"),
                    _ => Refusal::ApprovalCancelled,
                })?)
            }
        }
    }
}

pub(crate) fn descriptors(problem: DescriptorProblem) -> Refusal {
    Refusal::Descriptors { problem }
}
