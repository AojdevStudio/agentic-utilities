//! Daemon-backed list, add, reveal, edit, and delete. The client holds only
//! metadata. Values move between the terminal or stdin and the daemon through
//! passed descriptors, so this process never reads one.

use crate::broker::client_human::open_terminal;
use crate::broker::meta::ProjectMeta;
use crate::broker::operation::{Operation, Outcome, SecretSelector, ValueChange, ValueSource};
use crate::broker::{default_socket_path, Client, ConnectOutcome};
use anyhow::{bail, Context, Result};
use std::io::IsTerminal;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::Path;

/// Connect to the daemon. Without one, every value command fails. The client
/// has no vault access of its own.
pub(crate) fn connect_daemon() -> Result<Client> {
    connect_daemon_at(&default_socket_path())
}

pub(crate) fn connect_daemon_at(path: &Path) -> Result<Client> {
    match Client::try_connect_to(path)? {
        ConnectOutcome::Connected(client) => Ok(client),
        ConnectOutcome::Unavailable => bail!(
            "hush needs the hush broker daemon, and none is reachable at {}. \
hush never reads the vault from the client. Start the daemon (see docs/adr/0002-hush-broker.md)",
            path.display()
        ),
    }
}

/// Send one operation and turn a refusal into an error.
fn operate(op: Operation, fds: &[BorrowedFd<'_>]) -> Result<Outcome> {
    match connect_daemon()?.operate(op, fds)? {
        Ok(outcome) => Ok(outcome),
        Err(refusal) => bail!("{refusal}"),
    }
}

fn selector(key: String, project: Option<String>) -> SecretSelector {
    SecretSelector::Key { key, project }
}

pub(crate) fn list(project: Option<&str>, json: bool) -> Result<()> {
    let entries = connect_daemon()?.catalog(project)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for entry in &entries {
            println!(
                "{}\t{}\t{}",
                entry.id,
                entry.key,
                entry.project.as_deref().unwrap_or("no project")
            );
        }
    }
    Ok(())
}

/// Piped stdin goes to the daemon as the value input. A terminal stdin means
/// the daemon prompts on the terminal instead.
pub(crate) fn add(key: String, project: Option<String>, note: Option<String>) -> Result<()> {
    let project = match project {
        Some(project) => project,
        None => pick_project(&connect_daemon()?.projects()?)?,
    };
    let stdin = std::io::stdin();
    let outcome = if stdin.is_terminal() {
        let tty = open_terminal()?;
        let op = Operation::Add {
            project,
            key,
            note,
            value: ValueSource::TtyPrompt,
        };
        operate(op, &[tty.as_fd()])?
    } else {
        let op = Operation::Add {
            project,
            key,
            note,
            value: ValueSource::InputFd,
        };
        operate(op, &[stdin.as_fd()])?
    };
    report(outcome);
    Ok(())
}

/// The value goes only to the terminal. Redirected stdout is refused first,
/// so `hush reveal ... | cat` never starts an approval.
pub(crate) fn reveal(key: String, project: Option<String>) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        bail!("hush reveal shows values only on a terminal, and stdout is redirected");
    }
    let tty = open_terminal()?;
    let op = Operation::Reveal {
        secret: selector(key, project),
    };
    report(operate(op, &[tty.as_fd()])?);
    Ok(())
}

pub(crate) struct EditOptions {
    pub key: String,
    pub project: Option<String>,
    pub new_key: Option<String>,
    pub stdin: bool,
    pub note: Option<String>,
}

/// Without `--stdin` the stored value is kept. With it, piped stdin is the
/// new value, or the daemon prompts on the terminal after approval.
pub(crate) fn edit(options: EditOptions) -> Result<()> {
    if options.new_key.is_none() && options.note.is_none() && !options.stdin {
        bail!("nothing to change. Pass --new-key, --note, or --stdin");
    }
    let tty = open_terminal()?;
    let stdin = std::io::stdin();
    let piped = options.stdin && !stdin.is_terminal();
    let value = match (options.stdin, piped) {
        (false, _) => ValueChange::Keep,
        (true, true) => ValueChange::Replace(ValueSource::InputFd),
        (true, false) => ValueChange::Replace(ValueSource::TtyPrompt),
    };
    let op = Operation::Edit {
        secret: selector(options.key, options.project),
        new_key: options.new_key,
        note: options.note,
        value,
    };
    let outcome = if piped {
        operate(op, &[tty.as_fd(), stdin.as_fd()])?
    } else {
        operate(op, &[tty.as_fd()])?
    };
    report(outcome);
    Ok(())
}

pub(crate) fn delete(key: String, project: Option<String>, yes: bool) -> Result<()> {
    if !yes {
        bail!("refusing to delete without --yes (deletion is permanent)");
    }
    let tty = open_terminal()?;
    let op = Operation::Delete {
        secret: selector(key, project),
    };
    report(operate(op, &[tty.as_fd()])?);
    Ok(())
}

fn report(outcome: Outcome) {
    match outcome {
        Outcome::Added { meta } => println!("created {} ({})", meta.key, meta.id),
        Outcome::Edited { meta } => println!("updated {} ({})", meta.key, meta.id),
        Outcome::Deleted { key, .. } => println!("deleted {key}"),
        Outcome::Revealed { .. } => {}
        Outcome::CopySent { key } => println!(
            "sent {key} to the terminal clipboard. Delivery is not confirmed. Clear it manually"
        ),
    }
}

fn pick_project(projects: &[ProjectMeta]) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("--project is required when not running interactively");
    }
    eprintln!("Select a project:");
    for (index, project) in projects.iter().enumerate() {
        eprintln!("  {}. {}", index + 1, project.name);
    }
    eprint!("> ");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let selection: usize = line.trim().parse().context("enter a number")?;
    projects
        .get(selection.wrapping_sub(1))
        .map(|project| project.id.clone())
        .context("invalid selection")
}
