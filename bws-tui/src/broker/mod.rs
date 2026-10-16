//! Broker plumbing. See docs/adr/0002-hush-broker.md.
//!
//! No type in this tree that can cross the socket holds a secret value.

pub(crate) mod approve;
pub(crate) mod audit;
pub(crate) mod backend;
mod bws_child;
mod bws_spawn;
mod children;
pub(crate) mod client;
pub(crate) mod client_human;
pub(crate) mod client_run;
mod console;
pub(crate) mod daemon;
#[cfg(test)]
mod fake_backend;
mod fdio;
mod fdpass;
mod fields;
pub(crate) mod hardening;
pub(crate) mod human;
pub(crate) mod identity;
mod input_pump;
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
mod request;
pub(crate) mod run_io;
pub(crate) mod run_loop;
mod run_output;
pub(crate) mod run_prepare;
mod run_program;
pub(crate) mod runspec;
mod sdk;
pub(crate) mod server;
mod slots;
pub(crate) mod spawn;
mod spawn_fd;
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
mod bws_child_tests;
#[cfg(test)]
mod cancel_harness;
#[cfg(test)]
mod cancel_tests;
#[cfg(test)]
mod children_tests;
#[cfg(test)]
mod console_tests;
#[cfg(test)]
mod daemon_harness;
#[cfg(test)]
mod delete_tests;
#[cfg(test)]
mod fdpass_tests;
#[cfg(test)]
mod fix_tests;
#[cfg(test)]
mod human_tests;
#[cfg(test)]
pub(crate) mod private_temp_harness;
#[cfg(test)]
mod pty_harness;
#[cfg(test)]
mod round2_tests;
#[cfg(test)]
mod run_boundary_tests;
#[cfg(test)]
mod run_flow_tests;
#[cfg(test)]
mod run_harness;
#[cfg(test)]
mod run_input_tests;
#[cfg(test)]
mod run_path_tests;
#[cfg(test)]
mod run_release_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod run_unit_tests;
#[cfg(test)]
mod sdk_cancel_tests;
#[cfg(test)]
mod sdk_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod token_read_tests;
#[cfg(all(test, target_os = "linux"))]
mod token_tests;
#[cfg(test)]
mod trust_tests;
#[cfg(test)]
mod tty_owner_tests;
#[cfg(test)]
mod write_tests;
