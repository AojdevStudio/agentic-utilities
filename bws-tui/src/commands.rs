use crate::{bws, stdin_value};
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use secrecy::SecretString;
use std::io::IsTerminal;

#[derive(Subcommand)]
pub(crate) enum Cmd {
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
        #[arg(long)]
        value: Option<String>,
        /// Read the new value from stdin. Explicit so note/key-only edits never block.
        #[arg(long, conflicts_with = "value")]
        stdin: bool,
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

fn scoped_project_id(project: &Option<String>) -> Result<Option<String>> {
    match project {
        None => Ok(None),
        Some(query) => {
            let projects = bws::list_projects()?;
            Ok(Some(bws::resolve_project(&projects, query)?.id.clone()))
        }
    }
}

fn add(
    key: String,
    project: Option<String>,
    value: Option<String>,
    note: Option<String>,
) -> Result<()> {
    let projects = bws::list_projects()?;
    let project_id = match &project {
        Some(query) => bws::resolve_project(&projects, query)?.id.clone(),
        None => pick_project_interactive(&projects)?,
    };
    let value = value
        .or(stdin_value()?)
        .context("no --value given and nothing piped on stdin")?;
    let created = bws::create_secret(
        &key,
        &SecretString::from(value),
        &project_id,
        note.as_deref(),
    )?;
    println!("created {} ({})", created.key, created.id);
    Ok(())
}

fn list(project: Option<String>) -> Result<()> {
    let project_id = scoped_project_id(&project)?;
    let projects = bws::list_projects()?;
    for secret in bws::list_secrets(project_id.as_deref())? {
        let name = secret
            .project_id
            .as_deref()
            .and_then(|id| projects.iter().find(|project| project.id == id))
            .map(|project| project.name.as_str())
            .unwrap_or("no project");
        println!("{}\t{}\t{}", secret.id, secret.key, name);
    }
    Ok(())
}

fn get(key: String, project: Option<String>) -> Result<()> {
    let project_id = scoped_project_id(&project)?;
    let secrets = bws::list_secrets(project_id.as_deref())?;
    let secret = bws::find_by_key(&secrets, &key, project_id.as_deref())?;
    println!("{}", secret.value);
    Ok(())
}

fn edit(options: EditOptions) -> Result<()> {
    if options.new_key.is_none()
        && options.value.is_none()
        && options.note.is_none()
        && !options.stdin
    {
        bail!("nothing to change — pass --new-key, --value, --stdin, or --note");
    }
    let piped = if options.stdin {
        Some(stdin_value()?.context("--stdin was set but stdin was empty or interactive")?)
    } else {
        None
    };
    let new_value = options.value.or(piped).map(SecretString::from);
    let project_id = scoped_project_id(&options.project)?;
    let secrets = bws::list_secrets(project_id.as_deref())?;
    let secret = bws::find_by_key(&secrets, &options.key, project_id.as_deref())?;
    let updated = bws::edit_secret(
        &secret.id,
        options.new_key.as_deref(),
        new_value.as_ref(),
        options.note.as_deref(),
    )?;
    println!("updated {} ({})", updated.key, updated.id);
    Ok(())
}

struct EditOptions {
    key: String,
    project: Option<String>,
    new_key: Option<String>,
    value: Option<String>,
    stdin: bool,
    note: Option<String>,
}

fn delete(key: String, project: Option<String>, yes: bool) -> Result<()> {
    if !yes {
        bail!("refusing to delete without --yes (deletion is permanent)");
    }
    let project_id = scoped_project_id(&project)?;
    let secrets = bws::list_secrets(project_id.as_deref())?;
    let secret = bws::find_by_key(&secrets, &key, project_id.as_deref())?;
    bws::delete_secret(&secret.id)?;
    println!("deleted {}", secret.key);
    Ok(())
}

pub(crate) fn run(command: Cmd) -> Result<()> {
    match command {
        Cmd::Add {
            key,
            project,
            value,
            note,
        } => add(key, project, value, note),
        Cmd::List { project } => list(project),
        Cmd::Get { key, project } => get(key, project),
        Cmd::Edit {
            key,
            project,
            new_key,
            value,
            stdin,
            note,
        } => edit(EditOptions {
            key,
            project,
            new_key,
            value,
            stdin,
            note,
        }),
        Cmd::Delete { key, project, yes } => delete(key, project, yes),
    }
}

fn pick_project_interactive(projects: &[bws::Project]) -> Result<String> {
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
