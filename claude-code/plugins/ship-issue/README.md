# ship-issue

Execute GitHub issues one at a time as vertical slices. One issue, one branch, one PR per issue, sequential. The skill exists to keep `to-issues`-style slices from being re-horizontalized during execution: scope discipline is the only thing that makes those slices worth anything.

## What it does

The skill runs a disciplined per-issue loop:

1. **Sync** with `main` so work starts from the latest merged code.
2. **Fetch** the issue (`gh issue view <number> --comments`) and read it fully.
3. **Verify blockers** are closed before touching code.
4. **Check for ambiguity** in the acceptance criteria, routing back to a planning skill if a decision is missing rather than guessing.
5. **Branch** from the issue (`issue-<number>-<short-slug>`).
6. **TDD each acceptance criterion**, one behaviour at a time, red to green to refactor.
7. **Stay inside the slice** — refactors and cleanups become new issues, not scope creep.
8. **Functionally verify** end-to-end and capture evidence for the PR body.
9. **Self-review** the diff against the issue, tests, and project docs.
10. **Open the PR** with the issue title, `Closes #<number>`, the checked acceptance-criteria list, and the evidence.
11. **Babysit the PR** to a merge decision, working review comments until they resolve.
12. **Close the loop** to a terminal state (merged, explicit PR-open handoff, or blocked) before starting the next queued issue.

## Trigger phrases

The skill activates on phrases like:

- "ship issue"
- "ship #N"
- "resolve #N"
- "work through these issues"
- "execute the issue queue"
- after `to-issues` hands off a queue of issues

## Bundled content

```
skills/ship-issue/
└── SKILL.md    # full procedural playbook — rules, completion states, agent delegation, process
```

## Prerequisites

- **`gh` CLI authenticated** (`gh auth login`) — used to fetch issues, comments, and PR state.
- **A GitHub remote** on the repo (`git remote -v` should show an `origin` pointing to GitHub).
- **`git` 2.30+** for modern subcommands.

## Companion skills (used when available)

These are referenced by the playbook but **not bundled**. The skill uses them when present and degrades gracefully when they are not:

- **`tdd`** — drives the inner red/green/refactor loop per acceptance criterion.
- **`gitworkflow`** — opens and labels the PR. Available as a plugin in this same marketplace.
- **`babysit-pr`** — owns the PR lifecycle (CI polling, failed-check repair, merge/alert). Available as a plugin in this same marketplace.
- **`greploop`** — works review-automation comments to a passing review confidence.
- **`grill-me` / `request-refactor-plan` / `grill-with-docs` / `improve-codebase-architecture`** — planning skills the playbook routes back to when an acceptance criterion is ambiguous or the scope turns out to be wrong mid-flight.

When a companion skill is unavailable, perform the equivalent step manually (e.g. open the PR with `gh pr create`, poll CI with `gh pr checks`).

## License

MIT — see repository LICENSE.
