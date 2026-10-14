//! Binds the passed descriptors to the roles an operation names and checks
//! each one before the daemon uses it.

use super::console::{Console, Interrupt};
use super::material::SecretMaterial;
use super::mutate::{invalid, material, read_input, MAX_VALUE};
use super::operation::{DescriptorProblem, FdRole, Refusal, ValueSource};
use super::peercred::PeerCredentials;
use super::tty::{check_terminal, check_value_input};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;

/// The passed descriptors, checked against the roles the operation names.
#[derive(Default)]
pub(crate) struct Slots {
    console: Option<Console>,
    value: Option<OwnedFd>,
}

impl Slots {
    pub(crate) fn bind(
        roles: &[FdRole],
        fds: Vec<OwnedFd>,
        peer: PeerCredentials,
    ) -> Result<Self, Refusal> {
        if fds.len() != roles.len() {
            return Err(descriptors(DescriptorProblem::WrongCount {
                expected: roles.len() as u8,
                received: fds.len().min(u8::MAX as usize) as u8,
            }));
        }
        let mut slots = Self::default();
        for (role, fd) in roles.iter().zip(fds) {
            match role {
                FdRole::Terminal => {
                    check_terminal(fd.as_fd(), peer).map_err(descriptors)?;
                    slots.console = Some(Console::new(fd));
                }
                FdRole::ValueInput => {
                    check_value_input(fd.as_fd()).map_err(descriptors)?;
                    slots.value = Some(fd);
                }
            }
        }
        Ok(slots)
    }

    pub(crate) fn console(&mut self) -> Result<&mut Console, Refusal> {
        self.console
            .as_mut()
            .ok_or(descriptors(DescriptorProblem::Malformed))
    }

    /// The new value, from the passed input or a hidden prompt.
    pub(crate) fn read_value(
        &mut self,
        source: ValueSource,
        prompt: &str,
        socket: &UnixStream,
    ) -> Result<SecretMaterial, Refusal> {
        match source {
            ValueSource::InputFd => match self.value.take() {
                Some(fd) => read_input(fd),
                None => Err(descriptors(DescriptorProblem::Malformed)),
            },
            ValueSource::TtyPrompt => {
                let line = self.console()?.read_hidden(prompt, MAX_VALUE, socket);
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
