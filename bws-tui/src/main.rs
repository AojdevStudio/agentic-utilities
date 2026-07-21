mod bws;
mod tui;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use std::io::{IsTerminal, Read};

#[derive(Parser)]
#[command(
    name = "bws-tui",
    version,
    about = "Interactive TUI and script-friendly wrapper around the Bitwarden `bws` CLI"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a secret. Value comes from --value or stdin (multiline-safe).
    Add {
        #[arg(long)]
        key: String,
        /// Project name or UUID. Prompts interactively if omitted and a TTY is present.
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    /// List secrets (id, key, project — never values).
    List {
        #[arg(long)]
        project: Option<String>,
    },
    /// Print a secret's value to stdout.
    Get {
        #[arg(long)]
        key: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Edit a secret's key, value, and/or note.
    Edit {
        #[arg(long)]
        key: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        new_key: Option<String>,
        /// New value; reads stdin if omitted and piped.
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        note: Option<String>,
    },
    /// Permanently delete a secret. Requires --yes.
    Delete {
        #[arg(long)]
        key: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None => tui::run(),
        Some(cmd) => run_cmd(cmd),
    }
}

/// Read a piped value from stdin. Errors instead of hanging when stdin is a TTY.
fn stdin_value() -> Result<Option<String>> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf)?;
    Ok(Some(buf.trim_end_matches('\n').to_string()))
}

fn scoped_project_id(project: &Option<String>) -> Result<Option<String>> {
    match project {
        None => Ok(None),
        Some(q) => {
            let projects = bws::list_projects()?;
            Ok(Some(bws::resolve_project(&projects, q)?.id.clone()))
        }
    }
}

fn run_cmd(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Add {
            key,
            project,
            value,
            note,
        } => {
            let projects = bws::list_projects()?;
            let project_id = match &project {
                Some(q) => bws::resolve_project(&projects, q)?.id.clone(),
                None => pick_project_interactive(&projects)?,
            };
            let value = match value {
                Some(v) => v,
                None => stdin_value()?.context("no --value given and nothing piped on stdin")?,
            };
            let created = bws::create_secret(
                &key,
                &SecretString::from(value),
                &project_id,
                note.as_deref(),
            )?;
            println!("created {} ({})", created.key, created.id);
        }
        Cmd::List { project } => {
            let pid = scoped_project_id(&project)?;
            let projects = bws::list_projects()?;
            for s in bws::list_secrets(pid.as_deref())? {
                let pname = s
                    .project_id
                    .as_deref()
                    .and_then(|sid| projects.iter().find(|p| p.id == sid))
                    .map(|p| p.name.as_str())
                    .unwrap_or("no project");
                println!("{}\t{}\t{}", s.id, s.key, pname);
            }
        }
        Cmd::Get { key, project } => {
            let pid = scoped_project_id(&project)?;
            let secrets = bws::list_secrets(pid.as_deref())?;
            let s = bws::find_by_key(&secrets, &key, pid.as_deref())?;
            println!("{}", s.value);
        }
        Cmd::Edit {
            key,
            project,
            new_key,
            value,
            note,
        } => {
            let piped = stdin_value()?;
            if new_key.is_none() && value.is_none() && note.is_none() && piped.is_none() {
                bail!("nothing to change — pass --new-key, --value, --note, or pipe a value");
            }
            let pid = scoped_project_id(&project)?;
            let secrets = bws::list_secrets(pid.as_deref())?;
            let s = bws::find_by_key(&secrets, &key, pid.as_deref())?;
            let new_value = value.or(piped).map(SecretString::from);
            let updated = bws::edit_secret(
                &s.id,
                new_key.as_deref(),
                new_value.as_ref(),
                note.as_deref(),
            )?;
            println!("updated {} ({})", updated.key, updated.id);
        }
        Cmd::Delete { key, project, yes } => {
            if !yes {
                bail!("refusing to delete without --yes (deletion is permanent)");
            }
            let pid = scoped_project_id(&project)?;
            let secrets = bws::list_secrets(pid.as_deref())?;
            let s = bws::find_by_key(&secrets, &key, pid.as_deref())?;
            bws::delete_secret(&s.id)?;
            println!("deleted {}", s.key);
        }
    }
    Ok(())
}

/// Minimal numbered picker for `add` without --project on a TTY. Not the full TUI.
fn pick_project_interactive(projects: &[bws::Project]) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("--project is required when not running interactively");
    }
    eprintln!("Select a project:");
    for (i, p) in projects.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, p.name);
    }
    eprint!("> ");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let n: usize = line.trim().parse().context("enter a number")?;
    projects
        .get(n.wrapping_sub(1))
        .map(|p| p.id.clone())
        .context("invalid selection")
}
