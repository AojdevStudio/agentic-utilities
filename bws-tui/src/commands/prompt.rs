//! Interactive project choice for `hush add`.

use crate::bws;
use anyhow::{bail, Context, Result};
use std::io::IsTerminal;

pub(super) fn pick_project_interactive(projects: &[bws::Project]) -> Result<String> {
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
