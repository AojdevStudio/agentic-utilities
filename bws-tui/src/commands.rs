use crate::broker::runspec::Delivery;
use crate::{audit, bws, retrieve, run_cmd, stdin_value};
use anyhow::{bail, Context, Result};
use clap::Subcommand;
use secrecy::SecretString;
use std::ffi::OsString;
use std::path::PathBuf;

mod prompt;
use prompt::pick_project_interactive;

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
    /// List secrets (id, key, project, note — never values).
    List {
        #[arg(long)]
        project: Option<String>,
        /// Emit JSON: [{id, key, project, note}] — the discovery surface for agents.
        #[arg(long)]
        json: bool,
    },
    /// Print a secret's value to stdout.
    Get {
        #[arg(long)]
        key: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Run a command with secrets delivered by the broker daemon. Output is masked.
    #[command(group(clap::ArgGroup::new("bindings").required(true).multiple(true).args(["secrets", "fds"])))]
    Run {
        /// Secret into the child's environment: KEY, or KEY=ENVNAME to remap. Repeatable.
        #[arg(long = "secret")]
        secrets: Vec<String>,
        /// Secret through a pipe: KEY or KEY=ENVNAME. The child reads fd ENVNAME_FD. Repeatable.
        #[arg(long = "fd")]
        fds: Vec<String>,
        #[arg(long)]
        project: Option<String>,
        /// Never read stdin; the child gets end of input at once, like `ssh -n`.
        #[arg(short = 'n', long)]
        no_stdin: bool,
        /// Command to run, after `--`.
        #[arg(last = true, required = true)]
        cmd: Vec<OsString>,
    },
    /// Alias of `run --secret` for one release. Same daemon code path.
    Exec {
        /// Secret to inject: KEY, or KEY=ENVNAME to remap. Repeatable.
        #[arg(long = "key", required = true)]
        keys: Vec<String>,
        #[arg(long)]
        project: Option<String>,
        /// Never read stdin; the child gets end of input at once, like `ssh -n`.
        #[arg(short = 'n', long)]
        no_stdin: bool,
        /// Command to run, after `--`.
        #[arg(last = true, required = true)]
        cmd: Vec<String>,
    },
    /// Rebuild the local metadata index (id/key/project/note — never values).
    Sync,
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
    /// Run the broker daemon. Hidden. See docs/adr/0002-hush-broker.md.
    #[command(hide = true)]
    Daemon {
        #[arg(long)]
        socket: Option<PathBuf>,
        #[arg(long)]
        token_file: Option<PathBuf>,
        #[arg(long)]
        audit_log: Option<PathBuf>,
        #[arg(long)]
        bws_path: Option<PathBuf>,
        /// Run policy file. Defaults to /etc/hush/hushd.toml.
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Debug builds only. Allow token and bws owned by the daemon user.
        /// Release builds omit this flag so a non-root daemon always refuses to start.
        #[cfg(debug_assertions)]
        #[arg(long)]
        dev_same_user: bool,
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
    audit::record("add", &[&key])?;
    let created = bws::create_secret(
        &key,
        &SecretString::from(value),
        &project_id,
        note.as_deref(),
    )?;
    println!("created {} ({})", created.key, created.id);
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
    audit::record("edit", &[&options.key])?;
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
    audit::record("delete", &[&key])?;
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
        Cmd::List { project, json } => retrieve::list(project.as_deref(), json),
        Cmd::Get { key, project } => retrieve::get(&key, project.as_deref()),
        Cmd::Run {
            secrets,
            fds,
            project,
            no_stdin,
            cmd,
        } => run_cmd::run_flags(
            &[
                (&secrets, "--secret", Delivery::Env),
                (&fds, "--fd", Delivery::Fd),
            ],
            project,
            no_stdin,
            cmd,
        ),
        Cmd::Exec {
            keys,
            project,
            no_stdin,
            cmd,
        } => run_cmd::run_flags(
            &[(&keys, "--key", Delivery::Env)],
            project,
            no_stdin,
            cmd.into_iter().map(OsString::from).collect(),
        ),
        Cmd::Sync => retrieve::sync(),
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
        #[cfg(debug_assertions)]
        Cmd::Daemon {
            socket,
            token_file,
            audit_log,
            bws_path,
            policy,
            dev_same_user,
        } => crate::broker::daemon::run_daemon(crate::broker::daemon::DaemonOptions {
            socket: socket.unwrap_or_else(crate::broker::default_socket_path),
            token_file,
            audit_log,
            bws_path,
            policy,
            dev_same_user,
        }),
        #[cfg(not(debug_assertions))]
        Cmd::Daemon {
            socket,
            token_file,
            audit_log,
            bws_path,
            policy,
        } => crate::broker::daemon::run_daemon(crate::broker::daemon::DaemonOptions {
            socket: socket.unwrap_or_else(crate::broker::default_socket_path),
            token_file,
            audit_log,
            bws_path,
            policy,
            dev_same_user: false,
        }),
    }
}
