# Issue tracker: GitHub

Issues and PRDs for this repo live as GitHub issues. Use the `gh` CLI for all operations.
GitHub is the source of truth for work tracking; do not create local markdown
task files unless `docs/agents/workflow.md` explicitly opts out of GitHub.

## Prerequisites

- Run `gh auth status` before attempting writes.
- If GitHub auth is missing or expired, stop and report the auth blocker instead of falling back to local markdown.
- Infer the repo from `git remote -v` — `gh` usually does this automatically when run inside a clone.
- Check `docs/agents/workflow.md` before creating issues so project-board, PR, review, and release conventions stay in sync.
- If `docs/agents/workflow.md` names a GitHub Project, add new issues to that project as part of issue creation.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.
