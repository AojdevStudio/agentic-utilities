//! `hush install` and `hush status`. One read-only observation feeds both.
//! Install turns it into a private plan, and one checked apply owner runs it.
//! See docs/adr/0002-hush-broker.md.

mod acl;
mod apply;
mod files;
mod host;
mod input;
mod layout;
mod model;
mod observe;
mod plan;
mod probe;
mod publish;
mod render;
mod status;

use anyhow::{bail, Result};
use apply::Halt;
use layout::Layout;
use observe::{observe, Binaries};
use std::path::PathBuf;

/// Native host for this platform.
#[cfg(target_os = "linux")]
fn native_host() -> Result<Box<dyn host::Host>> {
    bail!("native Linux host is not wired yet")
}

#[cfg(target_os = "macos")]
fn native_host() -> Result<Box<dyn host::Host>> {
    bail!("native macOS host is not wired yet")
}

/// Status reports the installed service's own executables when a definition
/// install wrote is present, not whichever hush binary the caller ran.
fn status_binaries(layout: &Layout) -> Result<Binaries> {
    let definition = layout::definition_path();
    let installed = files::open_node(layout, &definition, false)
        .ok()
        .and_then(files::read_bounded)
        .and_then(|bytes| render::installed_binaries(&bytes));
    Ok(match installed {
        Some((hush, bws)) => Binaries { hush, bws: Some(bws) },
        None => Binaries { hush: std::env::current_exe()?, bws: None },
    })
}

pub(crate) fn status() -> Result<()> {
    let layout = Layout::production();
    let mut host = native_host()?;
    let observed = observe(&layout, &status_binaries(&layout)?, host.as_mut());
    print!("{}", status::Status::from_observed(&observed).render());
    Ok(())
}

pub(crate) fn install() -> Result<()> {
    // SAFETY: geteuid takes no arguments and cannot fail.
    if unsafe { libc::geteuid() } != 0 {
        bail!("hush install must run as root. Stage a root-owned hush, then run it with sudo");
    }
    let layout = Layout::production();
    let exe: PathBuf = std::fs::canonicalize(std::env::current_exe()?)?;
    let binaries = Binaries { hush: exe, bws: None };
    let mut host = native_host()?;
    match apply::install_with(&layout, &binaries, host.as_mut(), &mut input::ProcessInput) {
        Ok(completed) if completed.is_empty() => println!("hush is already installed. Nothing changed."),
        Ok(completed) => {
            for step in &completed {
                println!("done: {step}");
            }
        }
        Err(halt) => bail!(halt_message(&halt)),
    }
    let observed = observe(&layout, &binaries, host.as_mut());
    print!("{}", status::Status::from_observed(&observed).render());
    Ok(())
}

fn halt_message(halt: &Halt) -> String {
    let refusals = |list: &[plan::Refusal]| {
        list.iter()
            .map(|r| format!("\n  {}: {}{}", r.subject, r.reason, if r.unknown { " (unknown)" } else { "" }))
            .collect::<String>()
    };
    let done = |completed: &[String]| {
        if completed.is_empty() {
            "No step completed.".to_string()
        } else {
            format!("Completed: {}.", completed.join(", "))
        }
    };
    match halt {
        Halt::Refused(list) => format!("hush install refused before any change:{}", refusals(list)),
        Halt::Changed { completed, refusals: list } => format!(
            "hush install stopped because the host changed after planning:{}\n{} Rerun hush install.",
            refusals(list),
            done(completed)
        ),
        Halt::Failed { completed, step, error } => format!(
            "hush install stopped at '{step}': {error}\n{} Rerun hush install after fixing the cause.",
            done(completed)
        ),
    }
}

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod fake_host;
#[cfg(test)]
mod files_tests;
#[cfg(test)]
mod publish_tests;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tree;
