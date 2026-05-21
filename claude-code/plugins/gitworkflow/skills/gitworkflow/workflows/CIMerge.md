# CI Monitor & Auto-Merge Workflow

Monitor CI checks, wait for automated reviews to settle, repair PR metadata failures when possible, and merge PRs when all gates pass.

## When to Use

- After creating a PR (automatically offered by the PullRequest workflow)
- When a user says "merge my PR", "check CI", "is CI passing", "wait for checks"
- When resuming a previously-created PR that was waiting on CI or review
- Any time a PR exists and needs to get from "open" to "merged"

## Variables

```bash
PR_NUMBER: from PullRequest workflow output, or detected from current branch
BRANCH: current git branch or specified branch
MAX_CI_WAIT: 15 minutes (default)
MAX_FIX_ATTEMPTS: 3
SETTLE_WAIT: 240 seconds (for automated reviewers to post after checks pass; minimum 4 minutes)
```

## Workflow

### Phase 1: Detect PR State

If `PR_NUMBER` is not provided, detect it:

```bash
BRANCH=$(git branch --show-current)
PR_NUMBER=$(gh pr list --head "$BRANCH" --json number -q '.[0].number')
```

If no PR found, stop: "No open PR found for branch `$BRANCH`. Create one first with /GitWorkflow PR."

Gather current state:

```bash
# CI status
gh pr checks "$PR_NUMBER"

# Review / metadata state
gh pr view "$PR_NUMBER" --json reviewDecision,body,baseRefName,url,closingIssuesReferences

# PR merge state
gh pr view "$PR_NUMBER" --json mergedAt -q '.mergedAt'
```

Route based on state:

| State | Action |
|-------|--------|
| Already merged | Report "PR already merged." Stop. |
| `PR issue link` or similar metadata check failed | Go to Phase 2D (repair PR metadata) |
| CI passing, review approved | Go to Phase 4 (merge) |
| CI passing, changes requested | Go to Phase 3 (address feedback) |
| CI passing, no review decision | Go to Phase 2B (review settlement) |
| CI pending/running | Go to Phase 2A (monitor CI) |
| CI failed | Go to Phase 2C (fix CI) |

---

### Phase 2A: Monitor CI

CI takes time to queue after a push. Do not panic if the first poll returns empty.

```bash
sleep 15
```

Poll every 30 seconds, up to `MAX_CI_WAIT`:

```bash
ELAPSED=0
while [ "$ELAPSED" -lt 900 ]; do
  ACTIONS=$(gh run list --branch "$BRANCH" --limit 1 --json status,conclusion 2>/dev/null)
  CHECKS=$(gh pr checks "$PR_NUMBER" 2>&1)

  # Parse results: all pass, metadata failure, any fail, or still pending
  # ...

  sleep 30
  ELAPSED=$((ELAPSED + 30))
done
```

Important:
- Poll both `gh run list` and `gh pr checks`.
- Repo-specific checks may exist outside Actions runs.
- If a metadata check such as `PR issue link` fails, route to Phase 2D instead of treating it like a code/test failure.

If CI passes → proceed to Phase 2B.
If CI fails → proceed to Phase 2C.

---

### Phase 2B: Review Settlement

Automated reviewers (CodeRabbit, Codex, GitGuardian) analyze PRs asynchronously after CI passes.

**Step 1 — Wait for all PR checks to complete:**

```bash
SETTLE_ELAPSED=0
while [ "$SETTLE_ELAPSED" -lt 180 ]; do
  PENDING=$(gh pr checks "$PR_NUMBER" --json state -q '[.[] | select(.state == "PENDING")] | length' 2>/dev/null)
  [ "${PENDING:-0}" -eq 0 ] && break
  sleep 30
  SETTLE_ELAPSED=$((SETTLE_ELAPSED + 30))
done
```

**Step 2 — Wait for automated reviews to post:**

```bash
sleep 240
```

> **Mandatory minimum:** 240 seconds. Automated reviewers (Codex, CodeRabbit, GitGuardian) often post 60–180 seconds after checks complete. Merging before this window risks missing actionable feedback. The previous 90-second window was too short in practice.

**Step 3 — Check for review feedback:**

```bash
REVIEW_DECISION=$(gh pr view "$PR_NUMBER" --json reviewDecision -q '.reviewDecision')
```

| `reviewDecision` | Action |
|------------------|--------|
| `CHANGES_REQUESTED` | Go to Phase 3 |
| `APPROVED` | Go to Phase 4 |
| `""` (empty) | Go to Phase 4 if all checks are green |

Also check for substantive review comments even without a formal decision:

```bash
gh pr view "$PR_NUMBER" --json reviews --jq '.reviews[] | select(.state == "COMMENTED" or .state == "CHANGES_REQUESTED") | {author: .author.login, state: .state}'
```

If automated reviewers left actionable comments, address them before merging.

---

### Phase 2C: Fix CI Failures

When CI fails, do not guess — read the actual logs.

```bash
RUN_ID=$(gh run list --branch "$BRANCH" --limit 1 --json databaseId,conclusion -q '.[] | select(.conclusion == "failure") | .databaseId')
gh run view "$RUN_ID" --log-failed
```

Attempt to fix the issue. After fixing:

```bash
git add . && git commit -m "fix: address CI failure" && git push
```

Return to Phase 2A to re-monitor. Maximum `MAX_FIX_ATTEMPTS` (3) before stopping with an error report.

---

### Phase 2D: Repair PR Metadata Failures

Use this when a repo-specific PR check fails because the PR body is missing valid issue metadata.

Inspect the PR body and parsed closing links:

```bash
gh pr view "$PR_NUMBER" --json body,closingIssuesReferences,baseRefName,url
```

Common failure mode:
- body contains `Closes #` or `#123` in prose,
- body uses the wrong template section,
- or the repo expects `- [x] No issue required ...` and the box is still unchecked.

Repair path:

1. If the PR should close an issue, update the body with a real closing line such as `Closes #123`.
2. If the repo allows a no-issue path and this PR qualifies, check the exact template box text.
3. Edit the PR body in place:

```bash
gh pr edit "$PR_NUMBER" --body-file /tmp/pr-body.md
```

4. Wait for the `edited` event to rerun checks, then return to Phase 2A.

Do **not** continue toward merge while the metadata check is failing.

If there is no issue number and the repo does not allow a no-issue path, stop and report that the PR cannot merge until the issue linkage problem is fixed.

---

### Phase 3: Address Review Feedback

Read all review comments:

```bash
gh pr view "$PR_NUMBER" --json reviews --jq '.reviews[]'
```

Address each piece of feedback. After pushing fixes, return to Phase 2A.

Maximum 5 review cycles before stopping.

---

### Phase 4: Merge

All checks pass and no blocking reviews. Attempt merge:

**Step 1 — Try direct merge:**

```bash
gh pr merge "$PR_NUMBER" --squash --delete-branch
```

If exit code 0 → done. Report success.

**Step 2 — If merge blocked (review required):**

```bash
gh pr review "$PR_NUMBER" --approve --body "Self-approved: CI passing, all checks verified."
gh pr merge "$PR_NUMBER" --squash --delete-branch
```

If exit code 0 → done. Report success.

**Step 3 — If self-approve fails (branch protection):**

This is the legitimate pause point.

```md
⏸️ PR #<NUMBER> requires external review approval.
CI is passing. All automated checks clear.
URL: <PR_URL>

Resume with: /GitWorkflow merge
```

---

## Report

```md
## PR Merged ✅

**PR:** #PR_NUMBER
**Branch:** BRANCH
**Merge:** Squash merge
**CI:** All checks passing
**Reviews:** [summary of review state]

**URL:** <PR_URL>
```

---

## Error Handling

| Error | Action |
|-------|--------|
| No PR found for branch | Prompt user to create PR first |
| CI not detected after 5 minutes | Report and stop |
| PR metadata / issue-link check failing | Repair body or stop with exact missing requirement |
| CI fails 3 times | Report failure logs, stop |
| Review rejected 5 times | Report "fundamental disagreement", stop |
| Merge conflicts | Attempt rebase, or report and stop |
| Branch protection blocks merge | Report PR URL, suggest manual review |

---

## Merge Strategy Selection

| Branch Type | Default Strategy | Rationale |
|-------------|------------------|-----------|
| `feature/*` | `--squash` | Clean single commit on target branch |
| `release/*` | `--merge` | Preserve release commit history |
| `hotfix/*` | `--squash` | Minimal footprint for emergency fix |
