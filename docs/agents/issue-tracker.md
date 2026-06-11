# Issue tracker: GitHub

Issues and PRDs for this repo live as GitHub issues in `AojdevStudio/agentic-utilities`.
GitHub is the source of truth for work tracking; do not create local markdown task files unless `docs/agents/workflow.md` explicitly opts out of GitHub.

## Prerequisites

- Run `gh auth status` before attempting writes.
- If GitHub auth is missing or expired, stop and report the auth blocker instead of falling back to local markdown.
- Infer the repo from `git remote -v`; inside this checkout, `gh` should resolve `AojdevStudio/agentic-utilities`.
- Check `docs/agents/workflow.md` before creating issues so project-board, PR, review, and release conventions stay in sync.
- Add new issues to the configured GitHub Project as part of issue creation.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..." --label "ready-for-agent"` or `--label "ready-for-human"`.
- **Read an issue**: `gh issue view <number> --comments`.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments`.
- **Comment on an issue**: `gh issue comment <number> --body "..."`.
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`.
- **Close**: `gh issue close <number> --comment "..."`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue in `AojdevStudio/agentic-utilities`, then add it to the `agentic-utilities Backlog` GitHub Project.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.
