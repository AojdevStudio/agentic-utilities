//! Broker plumbing. See docs/adr/0002-hush-broker.md.
//!
//! No type in this tree that can cross the socket holds a secret value.

pub(crate) mod approve;
pub(crate) mod audit;
pub(crate) mod backend;
pub(crate) mod client;
pub(crate) mod client_human;
pub(crate) mod client_run;
mod console;
pub(crate) mod daemon;
#[cfg(test)]
mod fake_backend;
mod fdio;
mod fdpass;
pub(crate) mod hardening;
pub(crate) mod human;
pub(crate) mod identity;
pub(crate) mod mask;
pub(crate) mod material;
pub(crate) mod meta;
mod mutate;
#[cfg(target_os = "macos")]
mod opendirectory;
pub(crate) mod operation;
#[cfg(target_os = "linux")]
mod pam;
pub(crate) mod paths;
pub(crate) mod peercred;
pub(crate) mod policy;
mod protocol;
pub(crate) mod run_io;
pub(crate) mod run_loop;
pub(crate) mod run_prepare;
pub(crate) mod runspec;
mod sdk;
pub(crate) mod server;
mod slots;
pub(crate) mod spawn;
pub(crate) mod token;
mod tty;

#[cfg(test)]
pub(crate) use backend::VaultBackend;
pub(crate) use client::{Client, ConnectOutcome};
pub(crate) use client_run::RunInputs;
#[cfg(test)]
pub(crate) use fake_backend::FakeBackend;
pub(crate) use paths::default_socket_path;
#[cfg(test)]
pub(crate) use protocol::{Frame, RequestFrame, ResponseFrame};

#[cfg(test)]
mod add_tests;
#[cfg(test)]
mod console_tests;
#[cfg(test)]
mod daemon_harness;
#[cfg(test)]
mod fdpass_tests;
#[cfg(test)]
mod fix_tests;
#[cfg(test)]
mod human_tests;
#[cfg(test)]
mod pty_harness;
#[cfg(test)]
mod round2_tests;
#[cfg(test)]
mod run_flow_tests;
#[cfg(test)]
mod run_harness;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod run_unit_tests;
#[cfg(test)]
mod sdk_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod trust_tests;
#[cfg(test)]
mod write_tests;
