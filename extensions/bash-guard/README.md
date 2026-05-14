# bash-guard

Pi extension that prompts before risky bash commands and protects sensitive file paths.

## What it guards

- Bash risks: catastrophic deletes, destructive disk tools, `dd if=/dev/zero`, repo deletion/exposure, and force pushes.
- Sensitive file tools: prompts on private key/credential paths and Claude settings writes/edits.
- Prompt choices: `Run once`, `Always allow` for the exact command/path in the current Pi session, or `Abort`.
- Subagents/headless sessions: hard-blocks catastrophic commands instead of trying to prompt.

## Install/use

From this repo:

```bash
pi -e ./extensions/bash-guard/index.ts
```

As part of the package, Pi loads it through `package.json#pi.extensions`.

For global local use, symlink it into Pi's global extension directory:

```bash
mkdir -p ~/.pi/agent/extensions
ln -s "$PWD/extensions/bash-guard" ~/.pi/agent/extensions/bash-guard
```

Then run `/reload` inside Pi.

## Flags

- `--bash-guard-auto-allow`: in no-UI mode, allow prompt-level findings. Catastrophic subagent blocks still apply.
- `--bash-guard-headless-allow-catastrophic`: dangerous escape hatch for CI-like cases where catastrophic headless blocks are intentionally allowed.

## Policy files

Loaded on every tool call, in this order:

1. `~/.pi/agent/bash-guard-policy.json`
2. `<repo>/.pi/bash-guard-policy.json`

Local policy extends global policy.

Example:

```json
{
  "bash": {
    "allow": [
      { "pattern": "^git status( --short)?$", "reason": "safe git inspection" }
    ],
    "prompt": [
      { "pattern": "^npm publish", "reason": "package publishing", "severity": "high" }
    ],
    "block": [
      { "pattern": "curl .*\\|.*sh", "reason": "remote shell execution" }
    ]
  },
  "paths": {
    "allow": [
      { "tool": "read", "pattern": "~/.ssh/*.pub", "reason": "public SSH keys are OK" }
    ],
    "prompt": [
      { "tool": "edit", "pattern": "./.env.example", "reason": "env example edit" }
    ],
    "block": [
      { "tool": "*", "pattern": "./.env", "reason": "local secrets file" }
    ]
  }
}
```

`bash.*.pattern` values are JavaScript regular expressions. `paths.*.pattern` values are glob-style path patterns with `*`, `**`, and `~` home expansion.

## Command

`/bash-guard` shows policy locations and current mode.
