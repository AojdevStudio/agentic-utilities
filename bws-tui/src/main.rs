mod broker;
mod commands;
mod human_cmd;
mod run_cmd;
mod tui;

use anyhow::Result;
use clap::Parser;
use commands::Cmd;

#[derive(Parser)]
#[command(
    name = "hush",
    version,
    about = "Interactive TUI and script-friendly client for the hush broker daemon over Bitwarden Secrets Manager"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        None => tui::run(),
        Some(command) => commands::run(command),
    }
}

#[cfg(test)]
mod cli_tests;
