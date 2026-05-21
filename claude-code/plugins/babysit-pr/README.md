# babysit-pr

Unattended PR shepherd for Claude Code. Push a branch, walk away — the skill polls CI every 3 minutes, auto-fixes red checks, validates and addresses reviewer comments, and either alerts you or merges automatically when everything is green.

## What it does

The skill runs as a single-tick loop powered by the Claude Code `CronCreate` tool:

1. **Push** — On first invocation, ensures the branch is pushed and a PR exists (creates one via `gh pr create --fill` if needed). Refuses to start if the working tree has uncommitted changes.
2. **Poll** — Every tick fetches fresh PR state and check conclusions via `gh pr view` and `gh pr checks`.
3. **Address comments** — Pulls unresolved CodeRabbit and human reviewer comments. Validates each comment's claim against the actual file/lines before applying. Posts a reasoned rebuttal for wrong-headed suggestions instead of silently ignoring them. Commits and pushes fixes; exits the tick early so CI re-runs on the new SHA.
4. **Auto-fix red CI** — When a check fails, pulls `gh run view --log-failed`, diagnoses the root cause, applies a surgical fix, verifies locally, commits, and pushes. No retry cap — loops until green.
5. **Merge or alert** — When CI is green and all comments are resolved:
   - With `--auto-approve`: squash-merges and deletes the branch.
   - Without `--auto-approve`: sends a notification and cancels the cron.

## Trigger phrases

The skill activates on phrases like:

- "babysit this PR"
- "watch the CI"
- "monitor CI until green"
- "shepherd PR"
- "auto-merge when ready"
- "fix CI in a loop"
- "address PR comments and merge"
- "push and watch"
- "hand-off PR until green"
- `/babysit-pr`

## Bundled content

```
skills/babysit-pr/
└── SKILL.md    # full procedural playbook — arguments, tick logic, CI fix loop
```

## Prerequisites

- **`gh` CLI authenticated** (`gh auth login`) — used for PR push, CI polling, comment resolution, and auto-merge.
- **A GitHub remote** on the repo (`git remote -v` should show an `origin` pointing to GitHub).
- **Claude Code harness with Cron tools** — the skill relies on `CronCreate`, `CronList`, and `CronDelete`. Without these tools available in the harness, the polling loop cannot be armed.
- `git` 2.30+ for `--force-with-lease` and modern subcommands.

> **Session lifetime:** The cron is session-only — it does not persist to disk. Closing the Claude Code session mid-PR stops the loop. Restart with `/babysit-pr --pr <N>` to resume.

## User configuration (optional)

Create `.claude/babysit-pr.local.md` in any repo you use this skill in to override defaults:

```markdown
# babysit-pr local config

poll_interval: 5m           # override the default 3-minute cron cadence
notification_channel: slack # how to alert when --auto-approve is not set (slack, desktop, custom)
default_merge_method: squash  # squash | merge | rebase
```

The skill reads this file when present. Without it, defaults are: 3-minute interval, no auto-approve, squash merge.

## Optional companion plugins

These plugins are **not bundled** but are used automatically when available:

- **`coderabbit:autofix`** — Knows the CodeRabbit per-thread approval flow; used when resolving CodeRabbit review threads.

If `coderabbit:autofix` is not installed, the skill falls back to the raw GraphQL `resolveReviewThread` mutation.

## What's NOT in this plugin

- **Personal notification endpoints.** The "alert" step references your configured notification channel. Wire it in `.claude/babysit-pr.local.md` — the skill does not bundle a specific notify command.
- **The `superpowers` private skill suite.** The source skill referenced `superpowers:receiving-code-review` and `superpowers:systematic-debugging` for comment validation and CI diagnosis. These are not public. The bundled SKILL.md inlines the same validation logic procedurally — no dependency on a private skill namespace.
- **A retry cap.** The CI-fix loop has no strike counter by design. The user cancels with `CronDelete <id>` if they want to step in. Configure a shorter `--interval` (e.g. `--interval 10m`) to reduce frequency while a hard problem is being diagnosed.

## License

MIT — see repository LICENSE.
