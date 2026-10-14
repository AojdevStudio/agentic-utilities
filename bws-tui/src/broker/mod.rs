//! Broker plumbing. See docs/adr/0002-hush-broker.md.
//!
//! No type in this tree that can cross the socket holds a secret value.

pub(crate) mod audit;
pub(crate) mod backend;
pub(crate) mod client;
pub(crate) mod daemon;
pub(crate) mod hardening;
pub(crate) mod identity;
pub(crate) mod mask;
pub(crate) mod material;
pub(crate) mod meta;
pub(crate) mod paths;
pub(crate) mod peercred;
pub(crate) mod policy;
mod protocol;
pub(crate) mod run_loop;
pub(crate) mod run_prepare;
pub(crate) mod runspec;
pub(crate) mod server;
pub(crate) mod spawn;
pub(crate) mod token;

#[cfg(test)]
pub(crate) use backend::{FakeBackend, VaultBackend};
pub(crate) use client::{Client, ConnectOutcome};
pub(crate) use paths::default_socket_path;
#[cfg(test)]
pub(crate) use protocol::{Frame, RequestFrame, ResponseFrame};

#[cfg(test)]
mod fix_tests;
#[cfg(test)]
mod round2_tests;
#[cfg(test)]
mod tests;
