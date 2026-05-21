---
name: harness-worktrees
description: "Set up parallel git worktrees of the same repo, one per coding harness (Claude Code, Codex CLI, Cursor, etc.), so multiple AI tools can edit/typecheck/test in parallel without trampling each other's node_modules, build artifacts, or branches. Audits the repo first for shared-state hazards (hardcoded ports, DB file paths, sockets, build dirs that would collide if two worktrees ran a live dev server simultaneously), drops a Gotcha block into CLAUDE.md/AGENTS.md so any agent in any worktree knows the rule, then creates work/<tool> branches, sibling worktrees at <repo-parent>/<repo-name>-<tool>/, and runs the right install command per stack. USE WHEN user says 'set up worktrees for my coding tools', 'I want a Claude / Codex / pi worktree of this repo', 'parallel worktrees per harness', 'harness worktrees', 'make this repo work for all 3 tools at once', 'split this repo per coding agent', 'I use multiple AI coding tools on the same codebase, set them up', or any variant where the goal is one-checkout-per-tool isolation. Also triggers when user wants to keep different coding harnesses from polluting each other's branches or node_modules. Distinct from repo structure design skills and agent-readiness auditing skills."
---

# Harness Worktrees

Set up sibling git worktrees of the current repo, one per coding harness, so Claude Code, Codex CLI, Cursor, and any other AI coding tool can work on the same codebase in parallel without stepping on each other's branches, `node_modules`, or build artifacts.

## When the user wants this

Users running multiple coding harnesses on the same repo want each tool to have its own checkout so:
- Branches per tool don't collide
- `node_modules`, `target/`, `dist/`, `.next/`, `__pycache__/` are isolated per worktree (this happens automatically — git worktree gives each its own working directory)
- One tool's in-flight WIP doesn't block another tool from doing typecheck/test/edit work
- Long-lived integration branches (`work/claude`, `work/codex`, `work/pi`, etc.) make it obvious which session owns which work

## Distinct from neighboring skills

- **Repo structure design skills** — design the structure of a repo. Use those for *how should this repo be organized*. This skill is downstream of that — it duplicates the existing layout for parallel use.
- **Agent-readiness auditing skills** — score a repo's agent-readiness against an 8-artifact stack. Use those for *is this repo set up for autonomous AI implementation*. This skill is orthogonal — it's about running multiple tools on whatever the repo already looks like.
- **Single-worktree ephemeral skills** — create a single ephemeral worktree for an isolated feature branch. This skill creates a *durable, multi-tool* layout, not a one-shot worktree.

## The three modes

The skill has three modes. Pick before doing anything else, by asking the user explicitly if it's not already clear from the conversation.

### Mode A — code-only parallel (default, zero code changes)

Three (or N) worktrees can edit, typecheck, lint, and run tests simultaneously. Only one worktree may run a *live* dev session (`bun run dev`, `npm run dev`, `cargo run`, `uv run`, etc.) at a time, because the repo's bootstrap code typically hardcodes ports / DB paths that would collide.

**Setup cost:** ~5 minutes. No code edits to the repo.

### Mode B — full parallel dev (requires repo code changes)

All N worktrees can run live dev simultaneously, each on its own port and its own DB / data dir. Requires the repo to thread environment overrides (`PORT`, `DB_PATH`, etc.) through its bootstrap. **The skill does NOT make those code changes.** It surfaces the exact files that would need editing and stops, asking the user to either (a) make the changes and rerun, or (b) downgrade to Mode A.

### Mode C — reset this worktree to latest main

Use when the user says they want the current worktree refreshed, reset, or synced to the latest `main` after a PR merge.

Run `scripts/reset-worktree-to-main.sh` from this plugin's skill directory. It dry-runs by default and only mutates the repo with `--confirm`.

Rules:

- Always run the dry-run first and read the plan.
- Do **not** `git switch main`; in multi-worktree repos, `main` may be checked out elsewhere.
- The reset target is `origin/main` by default, and the current branch name is preserved.
- Refuse dirty worktrees unless the user explicitly asks to stash first; then use `--confirm --stash`.
- Never force-push, delete a remote branch, or assume the PR branch is safe to discard without the user's reset request.

Commands (run from the repo where you want the reset):

```bash
# dry run
${CLAUDE_PLUGIN_ROOT}/skills/harness-worktrees/scripts/reset-worktree-to-main.sh

# execute when the user explicitly wants this worktree reset
${CLAUDE_PLUGIN_ROOT}/skills/harness-worktrees/scripts/reset-worktree-to-main.sh --confirm

# execute while preserving dirty local changes in a stash
${CLAUDE_PLUGIN_ROOT}/skills/harness-worktrees/scripts/reset-worktree-to-main.sh --confirm --stash
```

## Workflow

### Step 1 — Pre-flight checks

Refuse to proceed if any of these are true (state the reason and stop):
- Not inside a git repo (`git rev-parse --is-inside-work-tree` fails)
- Working tree is dirty (`git status --porcelain` is non-empty) — offer to stash or abort
- Current HEAD is detached — ask the user to check out a branch first
- Any target worktree path already exists and is not empty

Confirm with the user:
- The list of tool names. Defaults: `["claude", "codex", "pi"]`. Accept any names; they're free-form (e.g. `cursor`, `windsurf`, `aider`).
- The repo's *integration branch* (the one worktrees branch off from). Default: the repo's HEAD branch (typically `main` or `master`).
- Whether to push the gotcha-doc commit to origin (default: yes — so all worktrees inherit it from the same commit).

### Step 2 — Audit shared-state hazards

Run `scripts/audit-hazards.sh <repo-path>` from this plugin's skill directory. The script greps for:
- **Hardcoded ports** — `localhost:\d+`, `:3000`, `:3001`, `:5173`, `process.env.PORT`, `Bun.env.PORT`, `Number(...) || \d+` patterns in `.ts`, `.tsx`, `.js`, `.rs`, `.py`, `.go`
- **DB / data-file paths** — `~/.<app>/`, `data.db`, `*.sqlite`, `chroma`, `pgdata`, `redis.sock`, `database.url`, `DB_PATH`, `DATABASE_URL` defaults
- **Socket files** — `.sock`, `IPC`, `Unix domain`
- **Build artifact dirs** — confirm `node_modules/`, `target/`, `dist/`, `.next/`, `build/`, `.venv/` exist and would be per-worktree (they will be, but flag any that are NOT in `.gitignore` and are committed)

The script outputs a structured report. Read it, and:
- If hazards point to specific files, show the user the file paths + line numbers and explain what would collide if two worktrees ran live at once.
- Decide whether to recommend Mode A or Mode B based on hazard density. If there are 3+ hardcoded resource handles (port, DB, sockets), Mode A is almost always the right default.

### Step 3 — Document the gotchas

This is the most important step. Append a Gotcha block to the cold-start brief at repo root (`CLAUDE.md` if present; otherwise `AGENTS.md`; otherwise create `CLAUDE.md`).

The block must be specific to *this repo's* hazards, not a generic warning. Use the audit output to fill in the file paths and resource names. Template:

```markdown
N. **Multi-worktree shared-state risk.** This repo is checked out as parallel `git worktree`s (`<repo>-claude/`, `<repo>-codex/`, `<repo>-pi/`) so different coding harnesses can work in parallel. The following resources are **shared across every worktree** because the bootstrap code uses fixed values:

- <hazard 1: e.g. SQLite DB at `~/.appname/data.db` — `packages/core/src/server.ts:143` calls `initDatabase()` with no arg>
- <hazard 2: e.g. dev server port 3001 — `packages/core/src/server.ts:107`>
- <hazard 3: ...>

Code-only work (typecheck, tests, lint, edits, commits) is safe to run in parallel — only one worktree may run a *live* dev session (`<dev-command>`) at a time. Before starting a live session, confirm no other worktree's process is up: `<port-check-command>`.

Until `<env-vars>` are threaded through the bootstrap, treat parallel live sessions as forbidden — they will corrupt shared state.
```

After appending, commit with a focused message (one line: `docs: document multi-worktree shared-state gotcha`) and push to origin so the worktrees, once created, all inherit the gotcha at the same commit.

If the user declined to push in Step 1, commit locally and warn them that worktrees created from the local commit will diverge from origin until they push.

### Step 4 — Create branches and worktrees

For each tool name (default `claude`, `codex`, `pi`):

1. `git branch work/<tool> <integration-branch>` — long-lived integration branch per tool
2. `git worktree add <repo-parent>/<repo-name>-<tool> work/<tool>` — sibling directory

`<repo-parent>` is the parent of the current repo's working directory. `<repo-name>` is the basename of the working directory. Example: if current repo is `~/Projects/my-app/`, the worktrees go at `~/Projects/my-app-claude/`, `~/Projects/my-app-codex/`, etc.

If a tool name conflicts with an existing branch, append a numeric suffix (`work/claude-2`) and warn the user.

### Step 5 — Per-stack install

In each new worktree, run the install command appropriate to the detected stack(s). Detect stacks by checking for these files in the worktree root:

| File present | Install command |
|---|---|
| `bun.lock` or `bun.lockb` | `bun install` |
| `pnpm-lock.yaml` | `pnpm install` |
| `yarn.lock` | `yarn install` |
| `package-lock.json` | `npm install` |
| `package.json` (no lockfile) | `npm install` (warn no lockfile) |
| `Cargo.toml` | `cargo fetch` (do NOT `cargo build` — that's slow, fetch is enough to populate `target/` index) |
| `pyproject.toml` + `uv.lock` | `uv sync` |
| `pyproject.toml` (no uv.lock) | warn — let user choose `uv sync`, `pip install -e .`, or skip |
| `requirements.txt` | warn — let user choose pip/uv/poetry |
| `Gemfile` | `bundle install` |
| `go.mod` | `go mod download` |

Run installs in parallel across worktrees (they're independent — no shared state). Capture exit codes and report any failures, but don't abort the other installs.

If a worktree has *multiple* stacks (e.g. a Tauri repo with both `bun.lock` and `Cargo.toml`), run all applicable install commands. The Rust install can be slow; consider running it last or in the background and reporting completion separately.

### Step 6 — Print the final map

End with a structured summary that the user can copy into a notes doc. Format:

```
WORKTREE MAP — <repo-name>

  <repo-parent>/
    ├── <repo-name>/                ← primary, branch: <integration-branch>
    │                                  (only place to git pull / merge to)
    │
    ├── <repo-name>-claude/         ← worktree, branch: work/claude
    ├── <repo-name>-codex/          ← worktree, branch: work/codex
    └── <repo-name>-pi/             ← worktree, branch: work/pi

  Gotchas (now in CLAUDE.md gotcha #N):
    - <hazard 1 one-liner>
    - <hazard 2 one-liner>
    - ...

  Rule: only one worktree runs a live dev session at a time.
  Before `<dev-command>`, run `<port-check-command>`.

  Workflow:
    1. work in <repo>-{tool}/ on branch work/{tool}
    2. push → PR → merge to <integration-branch> on GitHub
    3. in primary <repo>/: git pull
    4. in each worktree: git rebase origin/<integration-branch>
```

## Failure modes & edge cases

- **Submodules** — if the repo has `.gitmodules`, `git worktree add` won't auto-init them in the new worktree. After creating each worktree, `cd` in and run `git submodule update --init --recursive`. Surface this in the audit step so the user knows it'll happen.
- **Already-checked-out branch** — `git worktree add` refuses to check out a branch already in use elsewhere. If `work/<tool>` exists from a prior run, either pick up the existing worktree (verify it's at the right path) or fail with a clear message.
- **Permission denied on parent dir** — if `<repo-parent>` is not writable, abort and ask the user where to put the worktrees instead. Don't silently fall back to any default location.
- **Massive `node_modules`** — the parallel installs duplicate disk usage by N. On a 1.5GB `node_modules`, three worktrees adds ~4.5GB. Surface the cost in the pre-flight check so the user can decide.
- **The bootstrap code already supports env overrides** — if the audit finds `process.env.SIDECAR_PORT ?? 3001` and similar patterns are already pervasive, mention Mode B is genuinely available to the user without code changes. Don't gate on Mode A reflexively.
- **No CLAUDE.md or AGENTS.md exists** — create `CLAUDE.md` with just the gotcha block and a one-line header. Don't fabricate other content. If the user objects, move the block to a project-local `docs/MULTI-WORKTREE.md` instead.

## Why this matters

The friction this skill removes is real: switching coding harnesses on the same repo without isolation means every tool fights over the working tree, branch HEAD, install state, and editor caches. With per-tool worktrees, each session has stable identity — `work/claude` is "the Claude session's branch," and any agent that wakes up in `<repo>-claude/` knows it's the Claude worktree. The gotcha-doc step is what makes the layout discoverable to *future* agents in *future* sessions: any Claude or Codex spawned in any of the worktrees reads the cold-start brief and sees the rule about not running parallel live dev. Without that, a fresh agent has no way to know.

The skill deliberately stays in its lane:
- Doesn't make code changes (Mode B requires the user to do that work themselves)
- Doesn't touch `.git/config` global settings
- Doesn't install or configure any specific harness — it only sets up the shared filesystem layout
