# Commit Workflow

Commit with submodule awareness, hook-aware strategy, conventional messages, and auto-push.

## Flags

| Flag | Effect |
|------|--------|
| `--no-verify` | Skip pre-commit hooks and validation |
| `--no-submodules` | Skip submodule processing |
| `--no-push` | Commit only, no push |
| `-m "..."` / `--message "..."` | Use this message instead of generating one |

No flag skips the audit and CI/hooks checks (non-blocking, nothing to opt out of). Commit never writes `CHANGELOG.md`: the `Unreleased` section is regenerated on `main` at release time (see Release), so branches cannot conflict on it.

## 1. Submodules first (unless `--no-submodules`)

If `.gitmodules` exists, find dirty submodules and, for each: commit its changes inside the submodule with a conventional message, push it, then return to the parent (whose pointer update rides in the parent commit).

| `git submodule status` signal | Meaning |
|-------------------------------|---------|
| `+<sha>` | New commits — parent commit updates the pointer |
| `-<sha>` | Uninitialized — `git submodule update --init` |
| `(modified/untracked content)` | Commit inside the submodule first |

**Complete when:** no submodule shows modified or untracked content.

## 2. Strategy and validation

Detect the commit strategy from the repo's formatting hooks:

| Hooks | Strategy |
|-------|----------|
| No formatting hooks | PARALLEL — multiple independent commits allowed |
| Formatting hooks (non-aggressive) | COORDINATED — stage and commit sequentially |
| Aggressive formatters (e.g. prettier --write) | HYBRID — stage all, let the hook format, single commit |

Unless `--no-verify`: run the project's own lint/check command, alert on staged sensitive files (block those) and files >1MB (warn).

## 3. Commit and push

Stage what belongs together (split into atomic commits when the diff mixes concerns: code vs docs vs tests). Message per the SKILL.md contract (emoji + type(scope) + `Co-Authored-By: AOJDevStudio`), or the user's `-m`. Then push, setting upstream if the branch has none — the full commit→push cycle runs without confirmation unless `--no-push`.

## 4. Dependency audit (non-blocking)

Invoke the **DependencyAudit** workflow. It reports open Dependabot alerts by severity or degrades to a one-line reason (disabled, 403, non-GitHub remote, no gh). It never blocks, delays, or aborts the commit.

## 5. CI + hooks presence (non-blocking)

If `.github/workflows/` lacks a PR-gate/release workflow, or no hook manager is configured (`lefthook.yml` / `.husky/` / `.pre-commit-config.yaml` / `core.hooksPath` all absent), surface the gap once and offer CISetup. Both present → stay silent. If lefthook is configured but not installed, run `lefthook install`.

## Report

```
## Commit Complete

**Strategy:** <PARALLEL|COORDINATED|HYBRID>
**Submodules:** <N processed | none>
**Pushed:** ✅ origin/<branch> (or ⏸️ --no-push)
**Security:** 🛡️ <N open alerts | ℹ️ audit skipped — reason>
**Repo hygiene:** ✅ CI + hooks present (or 🧩 gap — /GitWorkflow CISetup)

**Commit:** <git log --oneline -1>
**Stats:** <git diff --stat HEAD~1>
```

## Error handling

| Error | Action |
|-------|--------|
| Clean working tree | Say so; nothing to commit |
| Pre-commit hook fails | Show the error, abort (unless `--no-verify`) |
| Submodule push fails | Warn, continue with the parent commit |
| Sensitive file staged | Block, show which |
| Audit or hygiene check failure of any kind | One line, continue (non-blocking) |
