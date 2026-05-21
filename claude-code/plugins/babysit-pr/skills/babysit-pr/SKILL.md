---
name: babysit-pr
description: "Babysit a PR end-to-end — push the branch, poll CI every 3 minutes, auto-fix red checks, address PR comments via validation, then alert (or auto-merge with --auto-approve) when ready. Use this whenever a user says 'babysit this PR', 'watch the CI', '/babysit-pr', wants a PR shepherded to merge, or wants CI failures auto-fixed in a loop."
---

# babysit-pr

Push the current branch, poll CI on a 3-minute cron, auto-resolve red CI and unresolved PR comments, then either alert the user or auto-merge depending on the `--auto-approve` flag. The loop runs until the PR is merged, the PR is closed, or the user cancels the cron.

This skill is procedural — every invocation does **one tick** of work, then re-arms itself via a direct `CronCreate` call. The same skill body handles the first tick (push + tick + arm cron) and every subsequent cron-fired tick (tick only). Do **not** wrap this in `/loop` — `/loop` calls `CronCreate` on every fire, which would spawn duplicate crons. Call `CronCreate` directly from this skill with `/babysit-pr --pr <N> [other flags]` as the fired prompt, and dedupe via `CronList` (see "Pre-flight" step 1).

## Arguments

Parse `$ARGUMENTS` for these flags:

| Flag | Effect |
|------|--------|
| `--auto-approve` | When CI is green and comments are resolved, run `gh pr merge --squash --delete-branch` without waiting. Without this flag, alert the user and stop the cron — they merge manually. |
| `--pr <N>` | Target a specific PR number. Default: infer from current branch via `gh pr view`. |
| `--interval <Nm>` | Override the 3-minute cadence. Default: `3m`. |

If no arguments are provided, treat as `babysit-pr` with defaults (no auto-approve, infer PR, 3-minute cadence).

## First-tick vs cron-fired tick

Both ticks share the same procedure below; the only difference is whether to do pre-flight (push, ensure PR) and whether to arm a new cron at the end. Detect which one this is at the start of every invocation:

1. Resolve the target PR number:
   - If `--pr <N>` was passed, use it.
   - Otherwise: `gh pr view --json number 2>/dev/null` from the current branch. If no PR exists yet, this is unambiguously a first tick — do pre-flight.

2. Call `CronList` and look for an active cron whose prompt starts with `/babysit-pr ` and includes `--pr <N>` for the resolved PR. If found, this is a **cron-fired tick** — skip pre-flight, run the per-tick procedure, do **not** arm another cron.

3. If no matching cron is found, this is the **first tick** — run pre-flight, then the per-tick procedure, then arm a cron (step "Arm the cron" below).

## Pre-flight (first tick only)

1. **Identify the PR.**
   - `git status -sb` to see the branch.
   - `gh pr view --json number,headRefName,state` to confirm the branch has an open PR.
   - If no PR exists: push the branch first (`git push -u origin <branch>`), then `gh pr create --fill` (use `--draft` if the user requested it).

2. **Refuse to start with a dirty tree.** A babysitter is autonomous — it cannot pause to ask "should I commit these unrelated edits?" mid-cron-fire. If `git status -sb` shows uncommitted changes on the first tick, refuse to arm the cron and tell the user to commit, stash, or discard them first. This is the only interactive guardrail; everything after this point runs unattended.

3. **Push committed-but-unpushed changes.** If `git log @{u}..` shows local commits ahead of origin, push them with `git push --force-with-lease` (`--force-with-lease` is critical — pure `--force` can silently clobber a teammate's push to the same branch).

## Arm the cron (first tick only, after the per-tick procedure runs)

If the PR is not yet merged or closed at the end of the first tick, arm the cron via `CronCreate`:

- `cron`: `*/3 * * * *` (or whatever was parsed from `--interval`)
- `prompt`: `/babysit-pr --pr <N> <other flags verbatim>` — embed the PR number even if the user didn't pass `--pr` explicitly, so `CronList` dedup works deterministically across multiple babysat PRs.
- `recurring`: `true`

Confirm to the user: cron ID, the 7-day auto-expiry, the cancel command (`CronDelete <id>`), **and that the loop dies when this Claude session closes** — the cron is session-only and not persisted to disk, so closing the terminal mid-PR will leave the branch un-merged until babysit-pr is restarted. Do **not** wrap the prompt in `/loop` — the loop skill calls `CronCreate` on every fire, which would spawn duplicate crons each tick.

## Per-tick procedure

Every tick — first or cron-fired — runs this procedure in order. Each step short-circuits the rest of the tick if it succeeds.

### 1. Refresh PR state

```
gh pr view <N> --json state,mergeable,mergeStateStatus,statusCheckRollup,reviewDecision
gh pr checks <N>
```

Capture: PR state, all check conclusions, and any unresolved review comments (next step). If state is `MERGED` or `CLOSED`, skip to "Stop conditions."

### 2. Address PR comments

Pull both CodeRabbit and human reviewer comments:

```
gh api repos/{owner}/{repo}/pulls/<N>/comments --jq '.[] | select(.in_reply_to_id == null)'
```

For **every unresolved comment**, validate before applying:

1. Read the comment's diff context and proposed change.
2. Verify the file and lines referenced, check whether the suggested fix is technically correct, and flag wrong-headed suggestions instead of blindly applying them. Do not agree with a comment just because a reviewer stated it confidently.
3. **Only if validation confirms the comment is correct**, apply the fix surgically (small, targeted edit — never rewrite a file because of one comment).
4. Resolve the thread:
   - CodeRabbit: use the `coderabbit:autofix` skill (optional companion — see README) which knows the per-thread approval flow.
   - Human reviewer: `gh api graphql -f query='mutation { resolveReviewThread(input: {threadId: "<id>"}) { thread { isResolved } } }'`. Get the threadId via `gh api graphql -f query='{ repository(owner:"X",name:"Y") { pullRequest(number:N) { reviewThreads(first:50) { nodes { id isResolved comments(first:1){nodes{databaseId}} } } } } }'`.
5. If validation finds the comment is wrong, post a reply explaining why instead of applying — don't silently ignore. Use `gh api -X POST repos/{owner}/{repo}/pulls/<N>/comments/<comment-id>/replies -f body='<one-paragraph rebuttal citing the file/line evidence that contradicts the suggestion>'` and then resolve the thread with the same `resolveReviewThread` mutation as above. The reply IS the resolution rationale — don't leave it as an open thread.

After applying any comment fixes: stage, commit (`fix(review): address <reviewer> feedback`), `git push --force-with-lease`. Bail out of this tick early — CI will re-run on the new SHA, so checking it now is wasted.

### 3. Triage CI status

If no comments needed addressing, look at `statusCheckRollup`:

| State | Action |
|-------|--------|
| Any check `IN_PROGRESS` or `QUEUED` | Report what's still pending and which checks are green. Stop the tick — the cron will fire again. |
| All checks `SUCCESS` | Move to "Merge or alert." |
| Any check `FAILURE` | Move to "Auto-fix red CI." |

### 4. Auto-fix red CI

When a check has failed, the design intent is "loop until fixed, no retry cap" — iterate until green:

1. Pull the failing job's logs:
   ```
   gh run view <run-id> --log-failed
   ```
2. Diagnose the root cause from the log. Don't guess — read the actual error. If the failure is non-obvious, read the logs more carefully before applying a guess-fix.
3. Apply a surgical fix to the failing code (small, targeted change — no rearchitecting, no scaffolding-deletes).
4. Verify locally before pushing — run the same command CI runs (e.g. `npm test`, `npm run typecheck`, `cargo check`). Skipping local verification just burns CI minutes.
5. Commit (`fix(ci): <one-line reason>`) and `git push --force-with-lease`.
6. Bail out of the tick. The next cron fire will re-check on the new SHA.

No retry cap — if the same fix attempt fails repeatedly, apply a *better* fix (read the log harder, diagnose more carefully), not a circuit breaker. The user cancels the loop manually with `CronDelete <id>` if they want to step in.

### 5. Merge or alert

CI is green and no unresolved comments remain. Branch on `--auto-approve`:

- **`--auto-approve` set:** `gh pr merge <N> --squash --delete-branch`. Confirm the merge SHA. Then go to "Stop conditions."
- **`--auto-approve` unset:** Send a notification via your configured notification channel (check `.claude/babysit-pr.local.md` for the channel — see README). Message: "PR #<N> '<title>' is green and clean — ready to merge. Run `gh pr merge <N> --squash --delete-branch` or say 'merge it'." Then go to "Stop conditions."

### 6. Stop conditions

After a merge, after the user is alerted, or after the PR is closed externally:

1. Find the babysit-pr cron via `CronList`.
2. `CronDelete <id>` to stop the loop.
3. Print a one-line summary so the cron-fire trace ends cleanly.

## Output format

Each tick should end with a brief summary. Examples:

**Pending tick:**
```
CHANGE: None this tick.
VERIFY: gh pr checks → 3/5 green; build-sidecar + lint-test still in_progress.
STATUS: Two checks left; next tick in ~3 min.
```

**Red-CI tick:**
```
CHANGE: Fixed lint-test failure in src/utils/validator.ts:42 (no-non-null-assertion); pushed abc1234.
VERIFY: Local typecheck clean; unit tests 11/11 pass; force-with-lease push succeeded.
STATUS: New SHA in flight — CI re-running, next tick in ~3 min.
```

**Merge tick (auto-approve):**
```
CHANGE: PR #42 squash-merged; cron cancelled; branch deleted on remote.
VERIFY: gh pr view → MERGED; merge SHA abc1234.
STATUS: Shipped.
```

## Gotchas

1. **Force-with-lease only.** Never `git push --force` from inside this loop — a teammate's concurrent push would be silently overwritten. `--force-with-lease` aborts safely if origin moved.
2. **Don't disable hooks.** Never use `--no-verify` to skip pre-commit hooks even if they're slow. If a hook fails, fix the underlying issue.
3. **Refuse to start with a dirty tree.** Pre-flight aborts on uncommitted changes. The skill only commits files it itself modified during a tick (CI fixes, comment-applied fixes), never pre-existing unrelated edits.
4. **One open babysit-pr cron at a time per PR.** Dedup via `CronList` matched against `--pr <N>` in the arming prompt. Multiple crons for the same PR = duplicate ticks = forced-push race conditions where two ticks try to commit fixes for the same red CI run simultaneously.
5. **CodeRabbit comments are nested replies.** Use `in_reply_to_id == null` to fetch only top-level threads, not the replies under them. Resolving the parent resolves the children.
6. **Validate before applying reviewer comments.** The validation step exists specifically to prevent performative agreement with reviewer comments that are wrong. Always check file/lines and whether the suggested fix is actually correct before applying — even when CodeRabbit looks confident. A wrong fix that ships is worse than a comment that stays open.
7. **Stop the cron on terminal states.** Merged, closed, and "alerted (no auto-approve)" are all terminal. Forgetting to `CronDelete` means the loop keeps firing after the user has moved on.
