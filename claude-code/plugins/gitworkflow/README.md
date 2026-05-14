# gitworkflow

Smart Git workflow engine for Claude Code. One skill routes between eight workflow playbooks — Commit, Branch, Release, PullRequest, CIMerge, Submodule, IssueAnalysis, DeployWorkflow — and adapts each to the repo's actual shape (default branch, `develop` presence, PR template, issue-link checks).

## What it does

- **Repo-aware branch targeting.** Detects the default branch and whether `develop` exists; feature branches PR into the right integration branch, releases/hotfixes target the default branch, and back-merges only happen when `develop` is real.
- **Hook-aware commits.** Picks PARALLEL / COORDINATED / HYBRID staging based on the repo's pre-commit hooks so aggressive formatters don't fight the commit pipeline.
- **CI monitoring + auto-merge.** Polls GitHub Actions, CodeRabbit, GitGuardian, and repo-specific checks; waits 240s for automated reviews to settle; auto-merges with squash once the decision is clear.
- **Issue analysis & routing.** `--issue-analysis` splits an open backlog into `<cut>-blocker` vs `post-<cut>`, assigns issues to coding-agent worktrees (claude / codex / pi / cursor) by warm context and file boundary, then writes labels and prints an ASCII route map.
- **Submodule-aware.** Detects dirty submodules and commits them before the parent so changes don't get orphaned.
- **Deploy-workflow isolation.** Pushes a GitHub Actions workflow file to `main` without merging the surrounding feature branch.
- **Changelog awareness.** Calls the optional companion `changelog` CLI to preview unreleased changes and generate CHANGELOG.md entries; skips gracefully if not installed.

## Trigger phrases

The skill auto-activates on phrases like:

- "commit my changes" / "make a commit"
- "create a feature branch" / "start a release"
- "create a PR" / "submit PR" / "open pull request"
- "merge my PR" / "wait for CI" / "is CI passing" / "monitor CI" / "auto-merge"
- "add a submodule" / "update submodules"
- `--issue-analysis` / "analyze the backlog" / "route issues across worktrees" / "who should work on what"
- "deploy this workflow to main" / "merge workflow only"

## Bundled content

```
skills/gitworkflow/
├── SKILL.md                  # entry point — routes to workflows/
├── workflows/                # one playbook per workflow
│   ├── Commit.md
│   ├── Branch.md
│   ├── Release.md
│   ├── PullRequest.md
│   ├── CIMerge.md
│   ├── Submodule.md
│   ├── IssueAnalysis.md
│   └── DeployWorkflow.md
└── templates/                # drop-in GitHub Actions
    ├── issue-labeler.yml     # deterministic keyword-based issue auto-labeler
    └── labeler-config.json   # config for issue-labeler
```

## Prerequisites

- `gh` CLI authenticated (`gh auth login`) — used by every workflow that interacts with GitHub.
- `git` 2.30+ for `git switch` and modern subcommands.

## Optional companion tools

These tools are **not bundled**; if present on PATH they are used automatically. If absent, the workflows skip the corresponding step.

- **`changelog` CLI** — Used by Commit and Release for unreleased-change previews and CHANGELOG.md generation. If you don't have one, the workflows still produce commits and releases, just without changelog automation.

## User configuration (optional)

Create `.claude/gitworkflow.local.md` in any repo you use this skill in to override defaults. Example:

```markdown
# gitworkflow local config

co_authored_by: My Team <team@example.com>
emoji_reference_file: ~/Documents/team-commit-emoji.yaml
changelog_cli: /opt/homebrew/bin/changelog
```

The skill reads this file when present and falls back to placeholders shown in `SKILL.md` otherwise.

## What's NOT in this plugin

- The author's full extended Git playbook (`AGENT.md` in the source repo, ~3300 words of methodology and conflict-resolution detail) is intentionally not bundled — the eight `workflows/*.md` files cover the operational surface area for a public plugin. If you want the full reference, see the source skill in the upstream repository.
- A hardcoded co-author string. Configure your own via the local config file above.

## License

MIT — see repository LICENSE.
