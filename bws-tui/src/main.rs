mod bws;
mod commands;
mod tui;

use anyhow::Result;
use clap::Parser;
use commands::Cmd;
use std::io::{IsTerminal, Read};

#[derive(Parser)]
#[command(
    name = "hush",
    version,
    about = "Interactive TUI and script-friendly wrapper around the Bitwarden `bws` CLI"
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

fn read_value(reader: &mut impl Read) -> Result<Option<String>> {
    let mut value = String::new();
    reader.read_to_string(&mut value)?;
    Ok((!value.is_empty()).then_some(value))
}

fn stdin_value() -> Result<Option<String>> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    read_value(&mut stdin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_value_preserves_multiline_bytes() {
        let mut input = Cursor::new("line one\nline two\n");
        assert_eq!(
            read_value(&mut input).unwrap().as_deref(),
            Some("line one\nline two\n")
        );
    }

    #[test]
    fn read_value_rejects_empty_stdin() {
        let mut input = Cursor::new("");
        assert_eq!(read_value(&mut input).unwrap(), None);
    }

    #[test]
    fn edit_stdin_is_explicit() {
        let cli =
            Cli::try_parse_from(["hush", "edit", "--key", "TOKEN", "--note", "rotated"]).unwrap();
        let Some(Cmd::Edit { stdin, .. }) = cli.cmd else {
            panic!("expected edit command");
        };
        assert!(!stdin);
    }

    #[test]
    fn rust_source_files_stay_within_the_repository_size_limit() {
        let mut pending = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let lines = std::fs::read_to_string(&path).unwrap().lines().count();
                    assert!(
                        lines <= 300,
                        "{} has {lines} lines; limit is 300",
                        path.display()
                    );
                }
            }
        }
    }
}
