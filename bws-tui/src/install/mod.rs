//! `hush install` and `hush status`. One read-only observation feeds both.
//! Install turns it into a private plan and applies it as root. Status prints it.

mod accounts;
mod acl;
mod apply;
mod custody;
mod facts;
mod host;
mod input;
mod layout;
mod linux;
mod macos;
mod native_host;
mod native_io;
mod native_program;
mod observe;
mod plan;
mod prereq;
mod publish;
mod reach;
mod status;
mod sys;

#[cfg(test)]
mod apply_harness;
#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod custody_tests;
#[cfg(test)]
mod fake_host_harness;
#[cfg(test)]
mod fixture_harness;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod linux_tests;
#[cfg(test)]
mod macos_tests;
#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod native_host_tests;
#[cfg(test)]
mod native_program_tests;
#[cfg(test)]
mod native_signal_tests;
#[cfg(test)]
mod observe_tests;
#[cfg(test)]
mod publish_tests;
#[cfg(test)]
mod reach_tests;
#[cfg(test)]
mod resume_tests;

pub(crate) use facts::Fact;

use anyhow::{anyhow, bail, Result};
use layout::Layout;
use native_host::NativeHost;
use observe::observe;
use plan::{Plan, Refusal};
use status::Status;

/// Print the six status facts. Needs no root, no daemon and no token.
pub(crate) fn status() -> Result<()> {
    let observation = observe(Layout::production()?, &mut NativeHost);
    print!("{}", Status(&observation));
    Ok(())
}

/// Install as root. Every known refusal comes before input or any write.
pub(crate) fn install() -> Result<()> {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } != 0 {
        bail!("hush install must run as root. Stage a trusted hush release in a root-owned path, then run that binary with sudo");
    }
    let mut host = NativeHost;
    let plan = Plan::from_observation(observe(Layout::production()?, &mut host)).map_err(
        |Refusal(reasons)| {
            anyhow!(
                "hush install refused before any change:\n{}",
                list(reasons.iter())
            )
        },
    )?;
    if plan.steps().is_empty() {
        println!("hush is already installed and running. Nothing changed.");
        return Ok(());
    }
    match apply::apply(plan, &mut host, &mut input::OperatorInput) {
        Ok(done) => {
            println!("hush install completed:\n{}", list(done.iter()));
            print!("{}", Status(&observe(Layout::production()?, &mut host)));
            Ok(())
        }
        Err(failure) => {
            if !failure.done.is_empty() {
                eprintln!("completed before stopping:\n{}", list(failure.done.iter()));
            }
            bail!("hush install stopped at: {} ({:#}). Completed steps stay. Run hush install again after fixing the cause", failure.step, failure.error)
        }
    }
}

fn list<T: std::fmt::Display>(items: impl Iterator<Item = T>) -> String {
    items
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}
