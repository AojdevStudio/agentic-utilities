# ReviewPR Workflow

Handle PR review feedback: read comments, address changes via subagent, re-push.

## Triggers
- "handle review", "PR feedback", "address review comments"
- "check PR review status on <TEAM>-123"
- Internally called by ResolveIssue Phase 9

## Prerequisites
- [ ] `gh` CLI installed and authenticated
- [ ] PR number or branch name known
- [ ] Repository path known

## Step 1: Check Review Status

```sh
gh pr view <PR_NUMBER> --json reviewDecision,reviews,state
```

| `reviewDecision` | Action |
|------------------|--------|
| `APPROVED` | PR is approved. Proceed to merge (or return to ResolveIssue Phase 10). |
| `CHANGES_REQUESTED` | Go to Step 2 to address feedback. |
| `""` (empty/pending) | Inform user: "PR is awaiting review. No action needed yet." Stop. |
| `REVIEW_REQUIRED` | Same as pending. Stop. |

## Step 2: Read Review Comments

Collect ALL review feedback:

```sh
# Get inline review comments (file-specific)
gh api repos/{owner}/{repo}/pulls/{pr_number}/comments \
  --jq '.[] | {path: .path, line: .line, body: .body, author: .user.login}'

# Get review body (top-level review comment)
gh pr view <PR_NUMBER> --json reviews \
  -q '.reviews | map(select(.state == "CHANGES_REQUESTED")) | .[-1] | {body: .body, author: .author.login}'
```

Compile all feedback into a structured list:
1. **File-specific comments** — path + line number + comment text
2. **General review comments** — top-level review body
3. **Requested changes summary** — distilled list of what needs to change

## Step 3: Address Feedback (Subagent)

Spawn a subagent to address ALL review feedback:

```
Agent tool:
  subagent_type: "general-purpose"
  model: "sonnet"
  prompt: |
    Address ALL PR review feedback on branch "$BRANCH_NAME" in repository at <REPO_PATH>.

    Review comments:
    <STRUCTURED FEEDBACK FROM STEP 2>

    Original acceptance criteria:
    <AC FROM THE LINEAR ISSUE>

    Instructions:
    1. For EACH review comment:
       a. Read the comment and the surrounding code
       b. Make the requested change
       c. Verify the change doesn't break existing tests
    2. After all changes:
       a. Run the full test suite
       b. Run linter if configured
       c. Commit as: fix(review): address PR feedback - <summary>
    3. Do NOT dismiss or skip any review comment
    4. Every comment must be addressed in code or explained
    5. Do NOT push — just commit locally
```

## Step 4: Push Changes

```sh
git push origin "$BRANCH_NAME"
```

## Step 5: Reply to Review Comments (optional)

For significant changes, post replies to review comments:

```sh
# Reply to a specific inline comment
gh api repos/{owner}/{repo}/pulls/{pr_number}/comments/{comment_id}/replies \
  -f body="Addressed: <what was done>"
```

## Step 6: Re-check Review Status

After pushing, the PR returns to pending review state.

Report:
```
Feedback addressed and pushed. Awaiting re-review.

Comments addressed: <N>
Changes made: <summary>
Tests: All passing
```

If called from ResolveIssue, return to Phase 9 wait state.

## Retry Limit

**Max 5 review-fix cycles.** After 5 rounds of changes-requested → fix → re-review, escalate:

```
PR #<NUMBER> has been through 5 rounds of review feedback.
This may indicate a fundamental design disagreement.
Please review the PR directly and consider whether the approach needs rethinking.
```

## Output Format

```
PR Review: #<PR_NUMBER>

Review Status:  Changes Requested → Addressed
Comments:       <N> inline, <N> general
Fixes:          <summary of changes made>
Tests:          All passing
Pushed:         Yes
Awaiting:       Re-review (round <N> of 5)
```
