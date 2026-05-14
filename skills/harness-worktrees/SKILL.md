---
name: harness-worktrees
description: Manage Superconductor/pi parallel git worktrees. Use when the user wants to reset, refresh, or sync the current worktree to latest main after a PR merges, says "mode C", or asks to get current with main. Preserves the worktree branch name and resets it to origin/main without switching to main.
---

# Harness Worktrees for Pi

Pi-native worktree maintenance for repos that use one git worktree per coding harness.

## Mode C: get this worktree current with main

When the user says any of these, treat it as an explicit instruction to make the current worktree match latest `origin/main`:

- "mode C"
- "get current with me"
- "get this worktree current"
- "reset this worktree to main"
- "refresh this worktree from main"
- "sync this worktree with latest main"
- "after merge, make this worktree current"
- "update this worktree after my PR merged"

Do **not** only acknowledge. Do **not** stop at dry-run. Do **not** ask for another confirmation for a clean worktree, even if the branch is ahead or behind.

Run the reset directly:

```bash
~/.pi/agent/skills/harness-worktrees/scripts/reset-worktree-to-main.sh --confirm
```

Then verify sync:

```bash
~/.pi/agent/skills/harness-worktrees/scripts/reset-worktree-to-main.sh
```

Report only the useful facts: branch, HEAD, ahead count, behind count, dirty status.

## Status-only check

If the user asks whether the worktree is behind, ahead, dirty, or in sync — without asking to get current — run dry-run only:

```bash
~/.pi/agent/skills/harness-worktrees/scripts/reset-worktree-to-main.sh
```

## Why this is not `git switch main && git pull`

In a multi-worktree setup, `main` may already be checked out in another worktree. `git switch main` can fail or violate ownership expectations.

The safe operation is:

1. stay on the current worktree branch;
2. fetch `origin`;
3. reset the current branch to `origin/main`.

This makes the worktree contents match latest main while preserving the worktree's branch identity.

## Dirty worktrees

The reset script refuses dirty worktrees unless `--stash` is passed.

For normal Mode C, run `--confirm` first. If it aborts because the worktree is dirty, report the dirty files and ask whether to preserve them with:

```bash
~/.pi/agent/skills/harness-worktrees/scripts/reset-worktree-to-main.sh --confirm --stash
```

Only use `--stash` when the user explicitly asks to preserve/stash local changes.

## Safety rules

- Do **not** switch to `main`.
- Do **not** force-push.
- Do **not** delete branches.
- Do **not** run `git reset --hard` manually for this workflow; use the script.
- For Mode C / get-current requests on a clean worktree, execute `--confirm` immediately.
- Refuse dirty worktrees unless the user explicitly asks to stash.
- Branch-local commits may be discarded by Mode C; that is expected when the user explicitly asks to get current with main.

## Expected result

The current branch points at latest `origin/main`; ahead is `0`, behind is `0`, and the worktree is clean.
