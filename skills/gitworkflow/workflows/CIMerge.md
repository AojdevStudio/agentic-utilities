# CI Monitor & Auto-Merge Workflow

The last mile: turn "PR created" into "PR merged" — monitor CI, let automated reviewers settle, repair metadata failures, merge when every gate passes.

**Spec-fidelity gate (required before any merge, 2026-08-06):** CI proves the code works, not that it's what was asked for. Before merging, diff the PR's result against the ORIGINATING spec — the issue's acceptance criteria, the ratified plan, the mock — and report every deviation or an explicit "no deviations". Deviations that expand or reshape scope need the user's yes before the merge proceeds. The `SpecFidelityGate` hook reminds once per PR per session; this paragraph is the standing requirement it points at.

## Route on current state

Detect the PR from the branch if not given (none open → stop and point at the PullRequest workflow). Then:

| State | Action |
|-------|--------|
| Already merged | Report and stop |
| Metadata check failing (e.g. `PR issue link`) | § Metadata repair |
| CI pending/running | § Monitor |
| CI failed | § Fix CI |
| CI passing, changes requested | § Address feedback |
| CI passing, approved or no decision | § Settle, then § Merge |

## Monitor

CI takes ~15s to queue after a push — an empty first poll is normal. Poll every 30s up to 15 minutes, checking **both** `gh run list` and `gh pr checks`: GitHub Actions, CodeRabbit, Codex, and GitGuardian register on different surfaces, and some appear only as PR checks. Conclude "no CI configured" only after 5 minutes of empty results from both — and then stop rather than merging unverified.

## Settle (mandatory before merge)

Automated reviewers post asynchronously *after* checks pass; merging before they finish defeats them. Wait for pending checks to clear, then hold a **240-second minimum settlement window** (90s proved too short in practice). Then read `reviewDecision` — `CHANGES_REQUESTED` → § Address feedback; approved or empty → check for substantive COMMENTED reviews too, address any actionable ones, then merge.

## Fix CI

Read the actual failed logs (`gh run view <run-id> --log-failed`) — never guess from the check name. Fix, push, return to § Monitor. Maximum 3 fix attempts, then stop with the failure report.

## Metadata repair

When a repo-specific check fails on PR metadata: inspect `body` and `closingIssuesReferences`. Typical causes — prose mention without a parseable closing link, wrong template section, an unchecked `- [x] No issue required` box. Fix the body in place (`gh pr edit --body-file`), let the edit re-run checks, return to § Monitor. No valid issue path exists → stop and report; never merge around a failing metadata check.

## Address feedback

Read all review comments, address each, push, return to § Monitor (CI re-runs on new commits). Maximum 5 review cycles, then stop and report the disagreement.

## Merge

Escalation ladder:

1. `gh pr merge --squash --delete-branch` (strategy per table below).
2. Blocked needing review → self-approve with a body noting CI is verified, retry the merge.
3. Branch protection still blocks → the one legitimate pause: report the PR URL and that external review is required; resume later with `/GitWorkflow merge`.

| Branch type | Strategy |
|-------------|----------|
| `feature/*`, `hotfix/*` | squash |
| `release/*` | merge commit (preserve history) |

## Report

```
## PR Merged ✅
**PR:** #N  **Branch:** <head> → <base>  **Merge:** squash
**CI:** all checks passing  **Reviews:** <summary>
**URL:** <url>
```
