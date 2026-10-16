//! CLI surface checks. The client holds no vault access and no value command.

use super::*;
use std::path::PathBuf;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(args)
}

#[test]
fn get_and_sync_are_gone() {
    for removed in ["get", "sync"] {
        let error = parse(&["hush", removed, "--key", "TOKEN"]).err().unwrap();
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
    }
}

#[test]
fn values_never_arrive_as_arguments() {
    for args in [
        ["hush", "add", "--key", "TOKEN", "--value", "placeholder"],
        ["hush", "edit", "--key", "TOKEN", "--value", "placeholder"],
    ] {
        let error = parse(&args).err().unwrap();
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

#[test]
fn exec_stays_an_alias_of_run() {
    let cli = parse(&["hush", "exec", "--key", "TOKEN", "--", "env"]).unwrap();
    let Some(Cmd::Exec { keys, cmd, .. }) = cli.cmd else {
        panic!("expected exec");
    };
    assert_eq!(keys, ["TOKEN"]);
    assert_eq!(cmd, ["env"]);
}

#[test]
fn reveal_and_metadata_edit_parse() {
    let cli = parse(&["hush", "reveal", "--key", "TOKEN", "--project", "API"]).unwrap();
    assert!(matches!(cli.cmd, Some(Cmd::Reveal { .. })));
    let cli = parse(&["hush", "edit", "--key", "TOKEN", "--note", "rotated"]).unwrap();
    let Some(Cmd::Edit { stdin, .. }) = cli.cmd else {
        panic!("expected edit command");
    };
    assert!(!stdin, "a note-only edit keeps the stored value");
}

#[test]
fn a_missing_daemon_is_a_hard_error() {
    let missing = PathBuf::from("/nonexistent/hush-test/hush.sock");
    let error = human_cmd::connect_daemon_at(&missing).err().unwrap();
    let message = error.to_string();
    assert!(
        message.contains("needs the hush broker daemon"),
        "{message}"
    );
    assert!(
        message.contains("never reads the vault from the client"),
        "{message}"
    );
}

#[test]
fn only_the_daemon_backend_spawns_bws() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![src.clone()];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if relative.starts_with("broker/") || relative == "cli_tests.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                !text.contains("Command::new"),
                "{relative} spawns a process"
            );
            assert!(
                !text.contains("arboard"),
                "{relative} touches the clipboard"
            );
        }
    }
    assert!(!src.join("bws.rs").exists());
    assert!(!src.join("index.rs").exists());
    assert!(!src.join("exec.rs").exists());
}

#[test]
fn rust_source_files_stay_within_the_repository_size_limit() {
    let mut pending = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
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

#[test]
fn run_and_exec_accept_the_no_stdin_flag() {
    for args in [
        ["hush", "run", "-n", "--secret", "TOKEN", "--", "cat"],
        [
            "hush",
            "run",
            "--no-stdin",
            "--secret",
            "TOKEN",
            "--",
            "cat",
        ],
        ["hush", "exec", "-n", "--key", "TOKEN", "--", "cat"],
        ["hush", "exec", "--no-stdin", "--key", "TOKEN", "--", "cat"],
    ] {
        let no_stdin = match Cli::try_parse_from(args).map(|cli| cli.cmd) {
            Ok(Some(Cmd::Run { no_stdin, .. } | Cmd::Exec { no_stdin, .. })) => no_stdin,
            Ok(_) => panic!("{args:?}: parsed as another command"),
            Err(error) => panic!("{args:?}: {error}"),
        };
        assert!(no_stdin, "{args:?}");
    }
    let cli = Cli::try_parse_from(["hush", "run", "--secret", "TOKEN", "--", "cat"]).unwrap();
    assert!(matches!(
        cli.cmd,
        Some(Cmd::Run {
            no_stdin: false,
            ..
        })
    ));
}
