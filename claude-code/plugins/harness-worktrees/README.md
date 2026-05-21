# harness-worktrees

Set up durable, per-harness git worktrees so Claude Code, Codex CLI, Cursor, and any other AI coding tool can work on the same repo in parallel — each with its own branch, `node_modules`, build artifacts, and session identity — without stepping on each other.

## What it does

The skill has three modes. Ask the user which they want if the request doesn't make it obvious.

### Mode A — code-only parallel (default, zero code changes)

Creates N sibling worktrees at `<repo-parent>/<repo-name>-<tool>/`, one per tool name. Each gets a long-lived `work/<tool>` branch and the right package install for the detected stack. A **Gotcha block** documenting the repo's shared-state hazards is appended to `CLAUDE.md` / `AGENTS.md` so every future agent in every worktree sees the rule. Only one worktree may run a live dev session at a time.

Setup cost: ~5 minutes. No code changes to the target repo.

### Mode B — full parallel dev (requires repo code changes)

All worktrees run live dev simultaneously, each on its own port and DB. The skill surfaces the exact files that need env-var threading and stops — the user makes the code changes, then reruns. If the user prefers not to make changes, downgrade to Mode A.

### Mode C — reset this worktree to latest main

Resets the current worktree's branch to `origin/main` (dry-run by default). Does NOT switch to `main` — main may be checked out in another worktree. Bundled script: `skills/harness-worktrees/scripts/reset-worktree-to-main.sh`.

## Trigger phrases

The skill activates on:

- "set up worktrees for my coding tools"
- "harness worktrees" / "parallel worktrees per harness"
- "one worktree per tool" / "per-harness worktree"
- "I want a Claude / Codex / Cursor worktree of this repo"
- "split this repo per coding agent"
- "I use multiple AI coding tools on the same codebase, set them up"
- "keep my coding tools from polluting each other"
- "reset worktree to main" / "refresh this worktree from main" / "sync worktree with latest main"

## Bundled content

```
skills/harness-worktrees/
├── SKILL.md                              # main skill — runs the 3-mode workflow
└── scripts/
    ├── audit-hazards.sh                  # scan repo for shared-state hazards (ports, DBs, sockets)
    └── reset-worktree-to-main.sh         # Mode C — reset current worktree branch to origin/main
```

### audit-hazards.sh

Scans source files (TS, JS, Rust, Python, Go) for:
- Hardcoded ports (`localhost:PORT`, `.listen(PORT)`, `PORT ?? NNNN`)
- DB / data-file paths (`*.sqlite`, `data.db`, `DATABASE_URL` defaults, `pgdata`, socket files)
- Generic socket / IPC files (`.sock`, `unix:///`, `AF_UNIX`)
- Build artifact dirs that are tracked in git (should be gitignored)
- Submodule declarations (each new worktree needs `git submodule update --init --recursive`)

Outputs a structured report and recommends Mode A or B based on hazard count. Uses `rg` when available, falls back to `grep`.

### reset-worktree-to-main.sh

Dry-runs by default. Flags: `--confirm` to execute, `--stash` to stash dirty changes before reset, `--remote` / `--branch` to target a different remote or branch.

## Prerequisites

- **git 2.30+** — uses `git worktree`, `git branch --show-current`, `git switch` semantics
- **Package manager** for your stack — auto-detected; supported:
  - `bun` (bun.lock / bun.lockb)
  - `pnpm` (pnpm-lock.yaml)
  - `yarn` (yarn.lock)
  - `npm` (package-lock.json or bare package.json)
  - `cargo` (Cargo.toml) — runs `cargo fetch`, not `cargo build`
  - `uv` (pyproject.toml + uv.lock)
  - `pip` / `poetry` (requirements.txt or pyproject.toml without uv.lock — user chooses)
  - `bundle` (Gemfile)
  - `go mod` (go.mod)
- **`rg` (ripgrep)** — optional but recommended for faster hazard scanning; falls back to grep
- **`gh` CLI** — optional; used by `reset-worktree-to-main.sh` to show the latest merged PR in the dry-run plan

## User configuration (optional)

Create `.claude/harness-worktrees.local.md` in any repo to override defaults:

```markdown
# harness-worktrees local config

# Tool names to create worktrees for (default: claude, codex, pi)
tools: [claude, codex, cursor]

# Integration branch to branch worktrees from (default: repo HEAD branch)
integration_branch: main

# Naming convention for worktrees (default: <repo-name>-<tool>)
# Example: my-app-claude, my-app-codex
worktree_naming: <repo-name>-<tool>
```

The skill reads this file when present and falls back to built-in defaults otherwise.

## What's NOT in this plugin

- **Code changes to thread env-vars** — Mode B surfaces the files that need editing, but does not write the code. The user does that work.
- **Harness configuration** — the skill sets up the filesystem layout, not the tool-specific config (`.cursor/`, `.codex/`, etc.).
- **Global git config changes** — nothing in `.git/config` is modified.
- **Automatic branch cleanup** — the `work/<tool>` branches are long-lived and intentionally left for the user to manage.

## License

MIT — see repository LICENSE.
