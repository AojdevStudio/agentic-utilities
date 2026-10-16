use crate::broker::runspec::Delivery;
use crate::{human_cmd, run_cmd};
use anyhow::Result;
use clap::Subcommand;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Create a secret. The daemon reads the value from piped stdin (multiline-safe)
    /// or prompts for it on the terminal with echo off. Refuses an existing key.
    Add {
        #[arg(long)]
        key: String,
        /// Project name or UUID. Prompts interactively if omitted and a TTY is present.
        #[arg(long)]
        project: Option<String>,
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
    /// Show a secret on this terminal after password approval. Never prints to stdout.
    Reveal {
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
    /// Edit a secret's key or note, or replace its value. Needs password approval.
    Edit {
        #[arg(long)]
        key: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        new_key: Option<String>,
        /// Replace the value. The daemon reads piped stdin, or prompts on the terminal.
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        note: Option<String>,
    },
    /// Permanently delete a secret. Requires --yes and password approval.
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

pub(crate) fn run(command: Cmd) -> Result<()> {
    match command {
        Cmd::Add { key, project, note } => human_cmd::add(key, project, note),
        Cmd::List { project, json } => human_cmd::list(project.as_deref(), json),
        Cmd::Reveal { key, project } => human_cmd::reveal(key, project),
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
        Cmd::Edit {
            key,
            project,
            new_key,
            stdin,
            note,
        } => human_cmd::edit(human_cmd::EditOptions {
            key,
            project,
            new_key,
            stdin,
            note,
        }),
        Cmd::Delete { key, project, yes } => human_cmd::delete(key, project, yes),
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
