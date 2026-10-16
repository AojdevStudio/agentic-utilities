//! Shared apply test helpers: a counting token source and one install call.

use super::apply::{apply, Failure};
use super::fake_host_harness::FakeHost;
use super::fixture_harness::Fixture;
use super::input::TokenSource;
use super::observe::observe;
use super::plan::{Plan, Step};
use anyhow::{bail, Result};
use secrecy::SecretString;

pub(crate) const SENTINEL: &str = "SYNTHETIC-install-token-not-real";

#[derive(Default)]
pub(crate) struct Token {
    pub reads: usize,
    pub fail: bool,
}

impl TokenSource for Token {
    fn read(&mut self) -> Result<SecretString> {
        self.reads += 1;
        if self.fail {
            bail!("input ended before a token arrived");
        }
        Ok(SecretString::from(SENTINEL))
    }
}

pub(crate) fn install(
    fixture: &Fixture,
    host: &mut FakeHost,
    token: &mut Token,
) -> Result<Vec<Step>, Failure> {
    let plan = Plan::from_observation(observe(fixture.layout(), host))
        .unwrap_or_else(|refusal| panic!("refused: {:?}", refusal.0));
    apply(plan, host, token)
}

/// Every file's bytes in the tree, joined, for searching a sentinel.
pub(crate) fn all_bytes(fixture: &Fixture) -> Vec<u8> {
    fixture
        .snapshot()
        .into_values()
        .flat_map(|(_, _, bytes)| bytes)
        .collect()
}
