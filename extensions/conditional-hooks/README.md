# conditional-hooks

`conditional-hooks` is a generic Pi Extension for user-configured **Conditional Hook** policy. It loads strict JSON config, reports what it found through `/conditional-hooks`, and runs only hooks that users explicitly configure.

The package does **not** hardcode worktree cleanup or any other hook behavior.

## Config locations

Use one or both config files:

- Global user config: `~/.pi/agent/conditional-hooks.json`
- Trusted project config: `.pi/conditional-hooks.json`

Project config is read only when the project is trusted by Pi. Project hooks merge by `name`: a trusted project config can add a hook, override a global hook with the same name, or disable a global hook by setting `enabled: false`.

Config must be strict JSON: no comments, no trailing commas, quoted property names, and valid JSON strings.

## Status command

Run:

```text
/conditional-hooks
```

The command reports loaded config sources, active hook names, disabled hook names, and parse/validation warnings. Invalid or unreadable config does not crash Pi; it appears as a warning.

## Worktree-GC example

Place this JSON in `~/.pi/agent/conditional-hooks.json` or in a trusted project's `.pi/conditional-hooks.json` to enable worktree cleanup after merge-shaped agent bash commands:

```json
{
  "hooks": [
    {
      "name": "worktree-gc-on-merge",
      "enabled": true,
      "events": ["tool_result", "tool_execution_end"],
      "when": {
        "toolName": { "regex": "(^|\\.)bash$|^Bash$" },
        "command": { "regex": "\\bgh\\s+pr\\s+merge\\b|\\bgit\\s+merge(?!-)\\b" },
        "repoContains": "scripts/worktree-gc.ts"
      },
      "run": {
        "command": "bun scripts/worktree-gc.ts --quiet",
        "cwd": "repoRoot",
        "timeout": 60000,
        "print": true,
        "appendToToolResult": true,
        "ignoreFailure": true
      }
    }
  ]
}
```

The example runs only in repositories containing `scripts/worktree-gc.ts`; `run.cwd: "repoRoot"` runs the command from the resolved Git repository root.

The merge regex is exactly `\\bgh\\s+pr\\s+merge\\b|\\bgit\\s+merge(?!-)\\b`. The `git\\s+merge(?!-)` part means `git merge-base --is-ancestor HEAD origin/main` must not trigger the hook.

## Disable in a trusted project

A trusted project can disable a same-named global hook:

```json
{
  "hooks": [
    {
      "name": "worktree-gc-on-merge",
      "enabled": false,
      "events": ["tool_result"],
      "when": {},
      "run": {}
    }
  ]
}
```
