# MonitorCI Workflow

Monitor GitHub Actions CI for a branch and fix failures via subagents.

## Triggers
- "check CI", "CI status", "is CI passing on my PR"
- "fix CI", "CI is failing"
- Internally called by ResolveIssue Phase 8

## Prerequisites
- [ ] `gh` CLI installed and authenticated
- [ ] Branch name or PR number known
- [ ] Repository path known (from `linear-context.json`)

## Step 1: Identify Branch

Determine the branch name from available context:
- If called with a branch name → use directly
- If called with a PR number → `gh pr view <NUMBER> --json headRefName -q '.headRefName'`
- If called with an issue identifier → branch is `feature/<ISSUE_ID_LOWERCASE>-*`

Verify the branch exists:
```sh
git ls-remote --heads origin "$BRANCH_NAME"
```

## Step 2: Initial Wait

**Wait 15 seconds after push before first poll.** CI takes time to queue — GitHub Actions, CodeRabbit, Codex, and claude-code review all need seconds to register. Polling immediately will return empty results.

```sh
sleep 15
```

## Step 3: Poll CI Status (Actions + PR Checks)

CI comes in two forms — **GitHub Actions runs** (`gh run list`) and **PR status checks** (`gh pr checks`). Some reviewers (CodeRabbit, Codex, GitGuardian) only register as PR checks, not Actions runs. You must monitor **both**.

### 3.1 Poll GitHub Actions runs

```sh
gh run list --branch "$BRANCH_NAME" --limit 1 --json status,conclusion,databaseId
```

### 3.2 Poll PR checks

```sh
gh pr checks "$PR_NUMBER" --json name,state,completedAt 2>/dev/null
```

PR checks report `state` as `SUCCESS`, `FAILURE`, `PENDING`, or `ERROR`.

### 3.3 Combined routing

| Actions Status | PR Checks Status | Action |
|---------------|-----------------|--------|
| `completed` + `success` | All `SUCCESS` | CI passed. Return success. Proceed. |
| `completed` + `failure` | Any | Go to Step 4 (fix failure). |
| Any | Any `FAILURE`/`ERROR` | Go to Step 4 (fix failure). |
| `in_progress`/`queued` | Any `PENDING` | Wait 30s, re-poll. Hard timeout 15 min. |
| No Actions runs | All checks `SUCCESS` | CI passed (repo uses checks only). Proceed. |
| No Actions runs | Any `PENDING` | Wait 30s, re-poll. Checks still processing. |
| No Actions runs | No checks (under 5 min) | CI hasn't queued yet. Keep polling. |
| No Actions runs | No checks (after 5 min) | Stop. Report "CI not detected." Do NOT merge. |

**CRITICAL: ALL production codebases have CI.** GitHub Actions, CodeRabbit, Codex, or claude-code review — every repo has automated checks. Empty results mean CI hasn't queued yet, NOT that CI is missing. Never skip CI monitoring. Never merge without CI verification.

**Polling implementation:**
```sh
TIMEOUT=900  # 15 minutes
ELAPSED=0
while true; do
  # Check Actions runs
  ACTIONS_STATUS=$(gh run list --branch "$BRANCH_NAME" --limit 1 --json status,conclusion -q '.[0].status')
  ACTIONS_CONCLUSION=$(gh run list --branch "$BRANCH_NAME" --limit 1 --json conclusion -q '.[0].conclusion')

  # Check PR checks (CodeRabbit, Codex, GitGuardian, etc.)
  PENDING_CHECKS=$(gh pr checks "$PR_NUMBER" --json state -q '[.[] | select(.state == "PENDING")] | length' 2>/dev/null)
  FAILED_CHECKS=$(gh pr checks "$PR_NUMBER" --json state -q '[.[] | select(.state == "FAILURE" or .state == "ERROR")] | length' 2>/dev/null)
  TOTAL_CHECKS=$(gh pr checks "$PR_NUMBER" --json state -q 'length' 2>/dev/null)

  # Any failed check → go to Step 4
  if [ "${FAILED_CHECKS:-0}" -gt 0 ]; then
    CONCLUSION="failure"
    break
  fi

  # Actions completed + all PR checks done
  if { [ "$ACTIONS_STATUS" = "completed" ] && [ "$ACTIONS_CONCLUSION" = "success" ]; } || \
     { [ -z "$ACTIONS_STATUS" ] && [ "${TOTAL_CHECKS:-0}" -gt 0 ]; }; then
    if [ "${PENDING_CHECKS:-0}" -eq 0 ]; then
      break  # All clear
    fi
  fi

  # Nothing found yet — under 5 min, keep waiting
  if [ -z "$ACTIONS_STATUS" ] && [ "${TOTAL_CHECKS:-0}" -eq 0 ] && [ "$ELAPSED" -lt 300 ]; then
    sleep 30
    ELAPSED=$((ELAPSED + 30))
    continue
  fi

  # Nothing found after 5 min — stop
  if [ -z "$ACTIONS_STATUS" ] && [ "${TOTAL_CHECKS:-0}" -eq 0 ] && [ "$ELAPSED" -ge 300 ]; then
    echo "CI not detected after 5 minutes. Stopping — do NOT merge without CI."
    exit 1
  fi

  if [ "$ELAPSED" -ge "$TIMEOUT" ]; then
    echo "CI timeout after 15 minutes"
    break
  fi

  sleep 30
  ELAPSED=$((ELAPSED + 30))
done
```

## Step 4: Read Failure Logs

```sh
RUN_ID=$(gh run list --branch "$BRANCH_NAME" --limit 1 --json databaseId -q '.[0].databaseId')
gh run view "$RUN_ID" --log-failed
```

Capture the full failure output for the subagent.

## Step 5: Fix CI Failure (Subagent)

Spawn a subagent to fix the CI failure:

```
Agent tool:
  subagent_type: "general-purpose"
  model: "sonnet"
  prompt: |
    Fix the CI failure on branch "$BRANCH_NAME" in repository at <REPO_PATH>.

    CI failure logs:
    <PASTE FAILURE LOGS>

    Instructions:
    1. Read the failing files identified in the logs
    2. Make minimal changes to resolve the failure
    3. Run tests locally: <test command from linear-context.json>
    4. If tests pass, commit as: fix(ci): resolve <failure description>
    5. Do NOT push — just commit locally
```

## Step 6: Push and Re-poll

After the subagent commits the fix:
```sh
git push origin "$BRANCH_NAME"
```

Return to Step 3 to re-poll CI.

## Retry Limit

**Max 3 fix attempts.** After 3 consecutive failures, stop and report:

```
CI FAILED after 3 fix attempts. Manual intervention required.

Branch:     $BRANCH_NAME
Run ID:     $RUN_ID
Attempts:   3/3

Latest failure:
<truncated failure logs>
```

## Output Format

```
CI Monitor: $BRANCH_NAME

Status:    Passing (attempt <N> of 3)
Run ID:    <run_id>
Duration:  <time from first poll to success>

Fixes Applied:
  1. <description of fix 1>
  2. <description of fix 2>
```
