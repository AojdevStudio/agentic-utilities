---
name: gitworkflow
description: Smart Git workflow engine — hook-aware commits, Git Flow branching, CI monitoring and auto-merge, changelog automation, submodule handling. USE WHEN committing, branching (feature/release/hotfix), cutting a release, opening or submitting a PR, merging or monitoring CI, setting up CI, auditing Dependabot alerts, or managing submodules.
metadata:
  category: git/pr
  lanes: [claude, codex, pi]
  author: ossie
context: fork
---

# git-workflow

Smart Git workflow engine: hook-aware commits, Git Flow branching, CI monitoring and auto-merge, changelog awareness, submodule handling.

## By Default

Invoking this skill makes a repo match a sane, low-friction workflow without being asked. Every **Commit**:

- **Auto-updates the CHANGELOG** (mandatory, no skip flag — a gate with an escape hatch isn't a gate): regenerates the `Unreleased` section in place, idempotently, via the bundled changelog tool.
- **Surfaces open Dependabot alerts** (never blocks — one-line reason and continue when unavailable).
- **Checks CI + local hooks exist** (never blocks — nudges toward CISetup when absent; runs `lefthook install` when configured-but-uninstalled).

## Changelog tool (tool-contract)

```bash
changelog --unreleased --force          # rewrite the Unreleased section (Commit does this every run)
changelog VERSION --auto --dry-run      # preview a release cut
changelog VERSION --auto --force        # write the release section
```

Install the CLI from `AojdevStudio/agentic-utilities#subdirectory=changelog` via `uv tool install`. Release semantics: cutting a release REPLACES the Unreleased section from commits — hand-written notes are previewed, then discarded. Gotcha (2026-07-30): `uv tool install --from <path>` silently reuses a cached build when the version string is unchanged; pass `--reinstall` when re-verifying local source changes.

## Workflow routing

Announce `Running the **<Workflow>** workflow from the **git-workflow** skill...`, then follow the file:

| Workflow | Trigger | File |
|----------|---------|------|
| **Commit** | commit | `workflows/Commit.md` |
| **Branch** | create/finish a feature, release, or hotfix branch | `workflows/Branch.md` |
| **Release** | create release, bump version, tag | `workflows/Release.md` |
| **PullRequest** | create/submit PR | `workflows/PullRequest.md` |
| **CIMerge** | merge PR, check/monitor CI, auto-merge | `workflows/CIMerge.md` |
| **CISetup** | set up/scaffold CI, GitHub Actions, runners | `workflows/CISetup.md` |
| **DependencyAudit** | dependabot alerts, security audit | `workflows/DependencyAudit.md` |
| **DeployWorkflow** | land a workflow file on main independent of feature work | `workflows/DeployWorkflow.md` |
| **IssueAnalysis** | analyze/route issues, plan a release cut | `workflows/IssueAnalysis.md` |
| **Submodule** | add/update/remove/sync submodules | `workflows/Submodule.md` |

git-workflow is the canonical head for GitHub delivery: if another skill or connector creates a PR, return to `workflows/PullRequest.md` and complete its metadata reconciliation — a PR sidebar may be empty only after sources were checked and reported none.

## Commit message format (contract)

```
<emoji> <type>(<scope>): <description>

[optional body — what and why]

Co-Authored-By: AOJDevStudio
```

Conventional Commits types; emoji per `templates/emoji-commit-ref.yaml`.

## Gotchas (verified)

- **Forked-execution merge loss (2026-07-13, a private repo PR #81).** Run as a forked execution, this skill's background CI poller dies with the forked context: checks go green and the merge silently never happens, leaving the PR open and MERGEABLE forever. The MAIN session owns the terminal merge step — verify the poller task still exists before trusting auto-merge, and on "No task found" re-check `gh pr checks` and merge directly. A completion claim from this skill is not evidence; `gh pr view --json state` is.
- **Local hooks are inert until `lefthook install` runs** (and the binary must exist: `bun add -d lefthook` or `brew install lefthook`). Writing `lefthook.yml` does nothing on its own; teammates cloning the repo must install too.
- **Vercel:** `lefthook install` in postinstall crashes there (not a git repo) — use `"postinstall": "lefthook install || true"`; and verify `NEXT_PUBLIC_SITE_URL` points at production, not localhost, on first deploy.
- Release-model gotchas (GITHUB_TOKEN tag suppression, auto-release XOR) live in `workflows/Release.md` § Posture. Runner/fork/org-auth gotchas live in `workflows/CISetup.md`. Dependabot scope gotcha lives in `workflows/DependencyAudit.md`.
