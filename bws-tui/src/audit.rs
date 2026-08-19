//! Append-only audit log of which secrets each hush invocation touched.
//! Names only, never values — forensics after an incident should be a grep,
//! not an agent's self-report.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

fn log_path() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?)
            .join(".local")
            .join("state"),
    };
    Ok(base.join("hush").join("audit.log"))
}

fn line(now: SystemTime, verb: &str, keys: &[&str]) -> String {
    let keys = if keys.is_empty() {
        "-".to_string()
    } else {
        keys.join(",")
    };
    format!("{} {verb} {keys}\n", humantime::format_rfc3339_seconds(now))
}

/// Audit failure fails the command: a log that silently stops recording is
/// worse than a loud error.
pub fn record(verb: &str, keys: &[&str]) -> Result<()> {
    let path = log_path()?;
    let dir = path.parent().context("audit path has no parent")?;
    std::fs::create_dir_all(dir)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open audit log at {}", path.display()))?;
    file.write_all(line(SystemTime::now(), verb, keys).as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn line_holds_timestamp_verb_and_key_names_only() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_755_500_000);
        assert_eq!(
            line(now, "exec", &["CF_TOKEN", "COOLIFY_API_TOKEN"]),
            "2025-08-18T06:53:20Z exec CF_TOKEN,COOLIFY_API_TOKEN\n"
        );
    }
}
