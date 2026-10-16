//! Binds the passed descriptors to the roles an operation names and checks
//! each one before the daemon uses it. A terminal becomes the daemon's own
//! private open of it. A value input is held unread until the operation needs
//! its value, then read only by a forked child.

use super::children::Children;
use super::console::{Console, Interrupt};
use super::fdpass::MAX_RIGHTS;
use super::fields::{checked_value, invalid, MAX_VALUE};
use super::input_pump::{read_input, InputStop};
use super::material::SecretMaterial;
use super::operation::{DescriptorProblem, FdRole, Refusal, ValueSource};
use super::peercred::PeerCredentials;
use super::request::RequestControl;
use super::spawn_fd::FdCeiling;
use super::tty::{check_terminal, check_value_input, reopen_terminal};
use std::os::fd::{AsFd, OwnedFd};

/// Where value-input readers come from.
#[derive(Clone, Copy)]
pub(crate) struct Readers<'a> {
    pub children: &'a Children,
    pub ceiling: FdCeiling,
}

/// The passed descriptors, checked against the roles the operation names.
pub(crate) struct Slots<'a> {
    control: &'a RequestControl<'a>,
    peer: PeerCredentials,
    readers: Readers<'a>,
    console: Option<Console<'a>>,
    value: Option<OwnedFd>,
}

impl<'a> Slots<'a> {
    pub(crate) fn bind(
        roles: &[FdRole],
        fds: Vec<OwnedFd>,
        peer: PeerCredentials,
        control: &'a RequestControl<'a>,
        readers: Readers<'a>,
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
            control,
            peer,
            readers,
            console: None,
            value: None,
        };
        for (role, fd) in roles.iter().zip(fds) {
            match role {
                FdRole::Terminal => {
                    let own = reopen_terminal(fd, peer)?;
                    slots.console = Some(Console::new(own, control));
                }
                FdRole::ValueInput => {
                    check_value_input(fd.as_fd()).map_err(descriptors)?;
                    slots.value = Some(fd);
                }
            }
        }
        Ok(slots)
    }

    /// The console, after checking again that the requester still owns the
    /// terminal and is in its foreground. Every prompt and every disclosure
    /// goes through here.
    pub(crate) fn console(&mut self) -> Result<&mut Console<'a>, Refusal> {
        let console = self
            .console
            .as_mut()
            .ok_or(descriptors(DescriptorProblem::Malformed))?;
        check_terminal(console.fd(), self.peer).map_err(descriptors)?;
        Ok(console)
    }

    /// The new value, from the passed input or a hidden prompt.
    pub(crate) fn read_value(
        &mut self,
        source: ValueSource,
        prompt: &str,
    ) -> Result<SecretMaterial, Refusal> {
        match source {
            ValueSource::InputFd => {
                let fd = self
                    .value
                    .take()
                    .ok_or(descriptors(DescriptorProblem::Malformed))?;
                let Readers { children, ceiling } = self.readers;
                let bytes =
                    read_input(fd, self.control, children, ceiling).map_err(input_refusal)?;
                checked_value(bytes)
            }
            ValueSource::TtyPrompt => {
                let line = self.console()?.read_hidden(prompt, MAX_VALUE);
                checked_value(line.map_err(|interrupt| match interrupt {
                    Interrupt::TooLong => invalid("the value is larger than 64 KiB"),
                    _ => Refusal::ApprovalCancelled,
                })?)
            }
        }
    }
}

fn input_refusal(stop: InputStop) -> Refusal {
    match stop {
        InputStop::Stopped(Interrupt::TimedOut) => {
            invalid("the value input did not finish in time")
        }
        InputStop::Stopped(_) => Refusal::ApprovalCancelled,
        InputStop::TooLong => invalid("the value is larger than 64 KiB"),
        InputStop::Busy => invalid("too many value inputs are still being read. Try again"),
        InputStop::Unreadable => invalid("the value input could not be read"),
    }
}

pub(crate) fn descriptors(problem: DescriptorProblem) -> Refusal {
    Refusal::Descriptors { problem }
}
