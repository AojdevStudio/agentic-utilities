//! `hush exec` — run a child process with named secrets injected as env vars.
//! The multi-secret delivery path: values go into the child's environment and
//! die with it, never touching argv, stdout, or disk.

use crate::{audit, retrieve};
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashSet;

/// One `--key` flag: `KEY` (env var named after the key) or `KEY=ENVNAME`.
#[derive(Debug, PartialEq)]
pub struct KeySpec {
    pub key: String,
    pub env: String,
}

fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn parse_spec(raw: &str) -> Result<KeySpec> {
    let (key, env) = match raw.split_once('=') {
        Some((key, env)) => (key, env),
        None => (raw, raw),
    };
    if key.is_empty() || env.is_empty() {
        bail!("invalid --key '{raw}': expected KEY or KEY=ENVNAME");
    }
    if !valid_env_name(env) {
        bail!("'{env}' is not a valid env var name — remap it with --key '{key}=ENVNAME'");
    }
    Ok(KeySpec {
        key: key.into(),
        env: env.into(),
    })
}

/// Two keys landing on the same env var would silently last-write-wins on a
/// credential; refuse before anything is fetched or spawned.
pub fn check_collisions(specs: &[KeySpec]) -> Result<()> {
    let mut seen = HashSet::new();
    for spec in specs {
        if !seen.insert(spec.env.as_str()) {
            bail!("env var '{}' is targeted by more than one --key", spec.env);
        }
    }
    Ok(())
}

pub fn run(raw_specs: &[String], project: Option<&str>, cmd: &[String]) -> Result<()> {
    let specs: Vec<KeySpec> = raw_specs
        .iter()
        .map(|raw| parse_spec(raw))
        .collect::<Result<_>>()?;
    check_collisions(&specs)?;
    let keys: Vec<&str> = specs.iter().map(|spec| spec.key.as_str()).collect();
    audit::record("exec", &keys)?;
    let mut env = Vec::with_capacity(specs.len());
    for spec in &specs {
        let secret = retrieve::fetch(&spec.key, project)?;
        env.push((spec.env.clone(), secret.value));
    }
    let (program, args) = cmd.split_first().context("no command given after --")?;
    spawn(program, args, env)
}

/// Replace this process with the command (unix), so secret values live only
/// in the child's env. Injected vars override inherited ones by design: the
/// caller named the key because they want BWS-truth, not a stale shell export.
pub fn spawn(program: &str, args: &[String], env: Vec<(String, String)>) -> Result<()> {
    let mut command = std::process::Command::new(program);
    command.args(args).envs(env);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(anyhow!(command.exec()).context(format!("failed to exec '{program}'")))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .map_err(|e| anyhow!(e).context(format!("failed to run '{program}'")))?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_defaults_env_name_to_the_key() {
        assert_eq!(
            parse_spec("CF_TOKEN").unwrap(),
            KeySpec {
                key: "CF_TOKEN".into(),
                env: "CF_TOKEN".into()
            }
        );
    }

    #[test]
    fn spec_remaps_with_key_equals_env() {
        assert_eq!(
            parse_spec("Cloudflare Admin=CF_ADMIN").unwrap(),
            KeySpec {
                key: "Cloudflare Admin".into(),
                env: "CF_ADMIN".into()
            }
        );
    }

    #[test]
    fn spec_rejects_empty_parts_and_invalid_env_names() {
        assert!(parse_spec("=CF").is_err());
        assert!(parse_spec("CF=").is_err());
        // A key with spaces is not a legal env var name without a remap.
        assert!(parse_spec("Cloudflare Admin").is_err());
        assert!(parse_spec("KEY=1BAD").is_err());
    }

    #[test]
    fn duplicate_env_targets_are_refused() {
        let specs = vec![
            parse_spec("A=SHARED").unwrap(),
            parse_spec("B=SHARED").unwrap(),
        ];
        assert!(check_collisions(&specs).is_err());
        // Same key fanned out to two names is harmless.
        let fan_out = vec![parse_spec("A").unwrap(), parse_spec("A=COPY").unwrap()];
        assert!(check_collisions(&fan_out).is_ok());
    }
}
