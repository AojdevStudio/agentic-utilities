# ResolveIssue Workflow

End-to-end issue resolution: fetch a Linear issue, branch, investigate, TDD, implement, PR, monitor CI, handle review, merge, and close. Fully autonomous from "Todo" to "Done".

## Triggers

- `/resolve <TEAM>-123` (preferred — fully autonomous, no re-triggering)
- "resolve issue", "resolve <TEAM>-123"
- "work on <TEAM>-123", "fix <TEAM>-123", "ship <TEAM>-123"
- "handle review on <TEAM>-123", "check PR status" (re-enters Phase 9)
- Any request to pick up and complete a Linear issue end-to-end

## Prerequisites

Before starting, ensure:
- [ ] `linearis` CLI is installed and authenticated
- [ ] `gh` CLI is installed and authenticated
- [ ] `LINEAR_API_TOKEN` is available in environment or `~/.env.secrets`
- [ ] Issue identifier is known (e.g., `<TEAM>-123`)
- [ ] `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json` is accessible for repo mapping

## Package Manager

**Always use `bun` — never `npm`.** All commands that would use `npm` must use `bun` instead:
- `bun run build` (not `npm run build`)
- `bun run test` (not `npm test`)
- `bun run lint` (not `npm run lint`)
- `bun install` (not `npm install`)

This applies to all phases and all subagent prompts. If a project's `package.json` has scripts, run them with `bun run <script>`. Subagent prompts MUST include this directive.

---

## Lifecycle Overview

```
Phase 1    Phase 2      Phase 3        Phase 4      Phase 5       Phase 6
Fetch  --> Branch   --> Investigate --> TDD Red  --> Implement --> Push
                        (Architect)    (Tests)      (Green)

Phase 7      Phase 8       Phase 9         Phase 10
--> PR   --> Monitor CI --> Handle Review --> Merge + Close
                            (wait state)
```

**Status transitions:**
```
Todo --> In Progress (Phase 2) --> In Review (Phase 9) --> Done (Phase 10)
```

---

## Phase 1: Fetch Issue

Retrieve full issue details and determine the target repository.

### 1.1 Read the issue

```bash
linearis issues read <ISSUE_ID>
```

Extract from the JSON output:
- `title` -- issue title
- `description` -- full description (may contain Acceptance Criteria)
- `id` -- issue UUID (needed for GraphQL operations)
- `identifier` -- human-readable ID (e.g., `<TEAM>-123`)
- `project.name` -- project name
- `labels` -- array of label names
- `priority` -- priority level

### 1.2 Parse acceptance criteria

The issue description should follow the template in `${CLAUDE_PLUGIN_ROOT}/skills/linear/IssueTemplate.md`. Extract:
- **Summary** section
- **Context** section (if present): Repository, GitHub, Relevant Files, Related Issues
- **Acceptance Criteria** items (AC-1, AC-2, AC-3)
- **Scope** boundaries (in-scope / out-of-scope)

If the description is unstructured or has no explicit acceptance criteria, synthesize testable acceptance criteria from the title and description. Max 3 criteria. Each must be binary pass/fail.

### 1.3 Look up repository path

Read `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json` and find the project entry matching `project.name`.

From the matching project, extract:
- `repoPath` -- local filesystem path
- `github` -- GitHub org/repo (e.g., `<org>/<repo>`)
- `triggers` -- for validation

**Decision gate:**

| Condition                                           | Action                                                                                                          |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `repoPath` is a valid path (e.g., `~/Projects/...`) | Continue to Phase 2                                                                                             |
| `repoPath` is `null`                                | Stop. Inform user: "This is a consulting project with no code repository. Resolution must be handled manually." |
| Project not found in JSON                           | Use AskUserQuestion to ask user for the repository path                                                         |

### 1.4 Validate sufficiency

Before proceeding, verify the issue has enough information to resolve:
- Title is present and actionable
- At least 1 acceptance criterion exists (extracted or synthesized)
- Repository path resolved successfully

If validation fails, stop and report what is missing.

---

## Phase 2: Status + Branch

Move the issue to "In Progress" and set up the working branch.

### 2.1 Update Linear status

```bash
linearis issues update <ISSUE_ID> -s "In Progress"
```

### 2.2 Navigate to repository

```bash
cd "${HOME}/<repoPath-with-tilde-expanded>"
```

Always expand `~` to `$HOME` when constructing paths.

### 2.3 Sync base branch

Determine whether the repo uses `develop` or `main` as its integration branch:

```bash
# Check if develop exists
git rev-parse --verify develop 2>/dev/null && echo "develop" || echo "main"
```

Then sync:

```bash
BASE_BRANCH="develop"  # or "main" if no develop
git checkout "$BASE_BRANCH"
git pull origin "$BASE_BRANCH"
```

### 2.4 Create feature branch

Branch name format: `feature/<ISSUE-ID>-kebab-case-title`

```bash
# Example: feature/<team>-123-add-login-form
BRANCH_NAME="feature/$(echo '<ISSUE_ID>' | tr '[:upper:]' '[:lower:]')-$(echo '<title>' | tr '[:upper:]' '[:lower:]' | tr ' ' '-' | tr -cd 'a-z0-9-' | head -c 50)"
git checkout -b "$BRANCH_NAME"
git push -u origin "$BRANCH_NAME"
```

Keep branch names under 60 characters total. Truncate the title slug if needed.

If the branch already exists, check it out instead:
```bash
git checkout "$BRANCH_NAME"
git pull origin "$BRANCH_NAME"
```

---

## Phase 3: Investigate (Subagent -- Architect)

Spawn a read-only agent to analyze the codebase and produce a TDD implementation plan.

### 3.1 Spawn Architect agent

Spawn an **Architect** subagent (subagent_type: `"Architect"`, model: `sonnet`) with the following context:

**Prompt payload:**
- Full issue description + acceptance criteria (from Phase 1)
- Repository path (absolute)
- Relevant files from Context section (if present in issue description)
- Instruction:

> Investigate this codebase at <REPO_PATH>. You are producing an implementation plan for the following issue:
>
> **Issue:** <ISSUE_ID> -- <title>
> **Acceptance Criteria:**
> - AC-1: <criteria>
> - AC-2: <criteria>
> - AC-3: <criteria>
>
> Do the following:
> 1. Read relevant source files (start with files listed in Context section, then explore as needed)
> 2. Understand the existing architecture, patterns, and conventions
> 3. Identify which files need modification or creation
> 4. Propose a TDD strategy: which tests to write, what assertions to make, mapped to each AC
> 5. Return a structured plan with two sections:
>    - **Test Plan:** which test files to create or modify, what each test verifies, mapped to AC
>    - **Implementation Plan:** which source files to create or modify, what changes are needed
>
> Consider existing test frameworks, patterns, and conventions already in use. Reference specific file paths.

### 3.2 Receive the plan

The Architect agent returns a structured investigation report containing:
- File list (files to modify/create)
- Test plan (test files, assertions, AC mapping)
- Implementation plan (source files, changes)

If the plan is unclear or incomplete, re-prompt the Architect for clarification before proceeding.

---

## Phase 4: TDD Red Phase (Subagent -- Write Failing Tests)

Spawn an agent to write failing tests that cover all acceptance criteria.

### 4.1 Spawn general-purpose agent

Spawn a **general-purpose** agent (subagent_type: `"general-purpose"`, model: `sonnet`) with:

**Prompt payload:**
- The investigation report from Phase 3
- Issue acceptance criteria
- Repository path (absolute)
- Branch name
- Instruction:

> Write failing tests for issue <ISSUE_ID> on branch "$BRANCH_NAME" in repository at <REPO_PATH>.
>
> **Acceptance Criteria:**
> - AC-1: <criteria>
> - AC-2: <criteria>
> - AC-3: <criteria>
>
> **Test Plan from Investigation:**
> <test plan from Phase 3>
>
> Instructions:
> 1. Create or modify test files as identified in the plan
> 2. Write tests that verify ALL acceptance criteria
> 3. Follow the existing test framework and patterns in this repo
> 4. Tests MUST fail initially -- this is the red phase of TDD
> 5. Run the test suite to confirm tests fail as expected. **Use `bun run test` (never npm).**
> 6. Commit the tests: `test(<scope>): add tests for <AC summary>`
> 7. Report: which test files were created, test names, and failure output
>
> **Package manager: Always use `bun`, never `npm`.** (`bun run test`, `bun run build`, `bun install`, etc.)

### 4.2 Verify red phase

Confirm the agent's output shows:
- Test files were created or modified
- Tests were executed
- Tests **failed** as expected (red phase confirms they test new behavior)

If tests pass immediately, they are not validating new behavior. Ask the agent to revise.

---

## Phase 5: Implement Green Phase (Subagent -- Make Tests Pass)

Spawn an agent to write the implementation that makes all tests pass.

### 5.1 Spawn general-purpose agent

Spawn a **general-purpose** agent (subagent_type: `"general-purpose"`, model: `sonnet`) with:

**Prompt payload:**
- The investigation report from Phase 3
- Test file paths and test names from Phase 4
- Issue acceptance criteria
- Repository path (absolute)
- Branch name
- Instruction:

> Implement the code to make ALL tests pass for issue <ISSUE_ID> on branch "$BRANCH_NAME" in repository at <REPO_PATH>.
>
> **Acceptance Criteria:**
> - AC-1: <criteria>
> - AC-2: <criteria>
> - AC-3: <criteria>
>
> **Implementation Plan:**
> <implementation plan from Phase 3>
>
> **Test Files:**
> <test file paths from Phase 4>
>
> Instructions:
> 1. Implement the code changes described in the plan
> 2. Do not modify the test assertions -- only write production code
> 3. Run the full test suite to verify ALL tests pass. **Use `bun run test` (never npm).**
> 4. Run the linter if configured: `bun run lint`
> 5. Run the build command if configured: `bun run build`
> 6. Commit the implementation: `<type>(<scope>): <description>`
> 7. Report: implementation summary, test results (pass/fail count), commit hash
>
> **Package manager: Always use `bun`, never `npm`.** (`bun run test`, `bun run build`, `bun install`, etc.)

### 5.2 Verify green phase

Confirm:
- All tests pass
- Linter passes (if applicable)
- Build succeeds (if applicable)

### 5.3 Retry on failure (max 2 retries)

If tests still fail after implementation:

| Attempt | Action |
|---------|--------|
| Retry 1 | Spawn a new general-purpose agent with: failure output, test file contents, implementation file contents. Instruct: "These tests are failing. Read the error output and fix the implementation. Do not modify tests." |
| Retry 2 | Spawn with additional context: include the investigation plan side by side. Instruct: "Final attempt. Here is every piece of context. Fix the implementation to pass all tests." |
| After 2 retries | **Stop.** Report failure to user with test output, implementation state, and error details. User must intervene. |

---

## Phase 6: Commit + Push

Verify all changes are committed and push to remote.

### 6.1 Verify commit state

Subagents in Phase 4 and Phase 5 should have committed their changes. Verify:

```bash
git status
```

If there are uncommitted changes, stage and commit them:

```bash
git add .
git commit -m "$(cat <<'EOF'
<type>(<scope>): <description>

<body explaining what changed and why>

Resolves <ISSUE_ID>
EOF
)"
```

Map issue labels to conventional commit type:

| Label | Commit Type |
|-------|-------------|
| Bug | `fix` |
| Feature | `feat` |
| Improvement | `refactor` |
| *(no label match)* | `feat` |

### 6.2 Push to remote

```bash
git push origin "$BRANCH_NAME"
```

If push is rejected (e.g., remote has diverged):

```bash
git pull --rebase origin "$BRANCH_NAME"
git push origin "$BRANCH_NAME"
```

Verify push succeeded by checking the exit code.

---

## Phase 7: Create PR

Open a pull request targeting the base branch.

### 7.1 Create pull request

```bash
gh pr create \
  --title "<type>(<scope>): <description>" \
  --base "$BASE_BRANCH" \
  --body "$(cat <<'EOF'
## Summary
<1-3 bullet points describing what this PR does>

## Linear Issue
Resolves <ISSUE_ID>: <issue title>

## Changes
- <bullet list of file-level changes>

## Test Plan
- [ ] <test description matching AC-1>
- [ ] <test description matching AC-2>
- [ ] <test description matching AC-3>

## Acceptance Criteria
- [ ] AC-1: <criteria>
- [ ] AC-2: <criteria>
- [ ] AC-3: <criteria>
EOF
)"
```

### 7.2 Extract PR details

```bash
PR_NUMBER=$(gh pr view --json number -q '.number')
PR_URL=$(gh pr view --json url -q '.url')
```

If PR creation fails, check if a PR already exists for this branch:

```bash
gh pr list --head "$BRANCH_NAME" --json number,url
```

If a PR exists, use it. If not, debug the creation failure (check `gh auth status`, verify branch is pushed).

**Do NOT update Linear status here.** Wait for CI to pass first (Phase 8).

---

## Phase 8: Monitor CI

Delegate CI monitoring to the MonitorCI workflow.

### 8.1 Follow MonitorCI workflow

Execute the steps in `${CLAUDE_PLUGIN_ROOT}/skills/linear/workflows/MonitorCI.md`:

1. Poll CI status for `$BRANCH_NAME`
2. Wait for completion (30-second intervals, 15-minute hard timeout)
3. If CI fails: MonitorCI spawns a general-purpose subagent to read failure logs and fix
4. Push fix, re-poll
5. Max 3 fix attempts

### 8.2 Evaluate CI outcome

| CI Result | Action |
|-----------|--------|
| Passing (attempt N of 3) | Record CI status. Proceed to Phase 9. |
| Timeout after 15 minutes | Stop. Report to user: "CI timed out. Check GitHub Actions directly." |
| Failed after 3 fix attempts | Stop. Report failure logs. User must intervene. |
| No CI runs found (first 5 min) | CI hasn't queued yet. Keep polling — ALL repos have CI (GitHub Actions, CodeRabbit, Codex, claude-code review). |
| No CI runs found (after 5 min) | Stop. Report "CI not detected after 5 minutes" to user. Do NOT skip to merge. |

---

## Phase 8.5: Review Settlement

After CI passes, automated reviewers (CodeRabbit, Codex, GitGuardian) need time to analyze the PR and post their reviews. Merging before they finish defeats the purpose of having them. This phase ensures all automated reviews have landed before proceeding to merge.

### 8.5.1 Wait for all PR checks to complete

PR checks include both CI runners and automated reviewers. Poll until none are pending:

```bash
# Poll every 30s until no PENDING checks remain (3-minute timeout)
SETTLE_ELAPSED=0
SETTLE_TIMEOUT=180
while true; do
  PENDING=$(gh pr checks "$PR_NUMBER" --json state -q '[.[] | select(.state == "PENDING")] | length' 2>/dev/null)

  if [ "${PENDING:-0}" -eq 0 ]; then
    break
  fi

  if [ "$SETTLE_ELAPSED" -ge "$SETTLE_TIMEOUT" ]; then
    echo "Review settlement timeout — $PENDING checks still pending after 3 minutes."
    break
  fi

  sleep 30
  SETTLE_ELAPSED=$((SETTLE_ELAPSED + 30))
done
```

### 8.5.2 Wait for automated reviews to post

Even after checks pass, some reviewers (like Codex) post PR review comments asynchronously. Wait 90 seconds after all checks clear, then check for any reviews with `CHANGES_REQUESTED`:

```bash
sleep 90

# Check if any automated reviews requested changes
REVIEW_DECISION=$(gh pr view "$PR_NUMBER" --json reviewDecision -q '.reviewDecision')
```

| `reviewDecision` | Action |
|------------------|--------|
| `CHANGES_REQUESTED` | Go to Phase 9.4 (address review feedback) |
| `APPROVED` | Proceed to Phase 9 |
| `""` / empty | Proceed to Phase 9 (no blocking reviews) |

### 8.5.3 Check for review comments

Even without a formal `CHANGES_REQUESTED` decision, check if any reviewer left comments that should be addressed:

```bash
gh pr view "$PR_NUMBER" --json reviews --jq '.reviews[] | select(.state == "COMMENTED" or .state == "CHANGES_REQUESTED") | {author: .author.login, state: .state, body: .body}'
```

If automated reviewers (CodeRabbit, Codex, claude-code) left substantive comments, spawn a subagent to address them before proceeding to merge.

---

## Phase 9: Handle PR Review

Update Linear status and manage the review cycle.

### 9.1 Update Linear status

```bash
linearis issues update <ISSUE_ID> -s "In Review"
```

### 9.2 Auto-merge or report

**If invoked from `/resolve` command:** Do NOT pause here. The command's Phase 9A handles auto-merge and self-approve automatically. Skip to Phase 10.

**If invoked standalone (not from `/resolve`):**
Update Linear to "In Review". Report PR URL and suggest re-entry via `/resolve <ISSUE_ID>` for autonomous completion.

### 9.3 Standalone wait state

**If invoked from `/resolve`, this phase is bypassed — `/resolve` handles auto-merge directly.**
If invoked standalone, the workflow can be re-entered via `/resolve <ISSUE_ID>`.

### 9.4 Re-entry: Handle review feedback

When re-entered (via "handle review on <TEAM>-123" or "check PR status"), follow the ReviewPR workflow at `${CLAUDE_PLUGIN_ROOT}/skills/linear/workflows/ReviewPR.md`:

1. Check review decision: `gh pr view <PR_NUMBER> --json reviewDecision,reviews,state`
2. Route based on decision:

| `reviewDecision` | Action |
|------------------|--------|
| `APPROVED` | Proceed to Phase 10. |
| `CHANGES_REQUESTED` | ReviewPR addresses feedback via subagent, pushes, re-enters wait state. |
| `""` / `REVIEW_REQUIRED` | Inform user: "PR is awaiting review. No action needed yet." Re-enter wait state. |

3. After addressing changes: push, report, return to wait state for re-review.

**Max 5 review cycles.** After 5 rounds of changes-requested, escalate:

```
PR #<NUMBER> has been through 5 rounds of review feedback.
This may indicate a fundamental design disagreement.
Please review the PR directly and consider whether the approach needs rethinking.
```

---

## Phase 10: Merge + Close

Merge the approved PR, clean up, and close the Linear issue.

### 10.1 Verify PR is approved

```bash
gh pr view <PR_NUMBER> --json reviewDecision -q '.reviewDecision'
```

Proceed only if `reviewDecision` is `APPROVED`. If not, return to Phase 9.

### 10.2 Merge PR

```bash
gh pr merge <PR_NUMBER> --squash --delete-branch
```

### 10.3 Handle merge conflicts

If merge fails due to conflicts:

```bash
git checkout "$BRANCH_NAME"
git fetch origin "$BASE_BRANCH"
git rebase "origin/$BASE_BRANCH"
```

If conflicts are simple (few files), resolve them directly.
If conflicts are complex (many files, logic changes), spawn a general-purpose subagent:

```
Agent tool:
  subagent_type: "general-purpose"
  model: "sonnet"
  prompt: |
    Resolve merge conflicts on branch "$BRANCH_NAME" in repository at <REPO_PATH>.
    The branch is being rebased onto "$BASE_BRANCH".

    Resolve all conflicts preserving the intent of the feature branch changes.
    Run tests after resolution to ensure nothing is broken.
    Commit the resolution. Do NOT push.
```

After resolving:

```bash
git push --force-with-lease origin "$BRANCH_NAME"
```

Re-attempt merge:

```bash
gh pr merge <PR_NUMBER> --squash --delete-branch
```

If merge is blocked by branch protection rules, report:
```
Merge blocked by branch protection. May need admin override or additional approvals.
```

### 10.4 Update Linear status

```bash
linearis issues update <ISSUE_ID> -s "Done"
```

### 10.5 Output completion report

```
Issue Resolved: <ISSUE_ID> -- <Title>

Branch:    <branch-name>
PR:        <PR URL>
Target:    <base branch>
Merged:    Yes (squash)
Linear:    Done

Tests:
  Written:  <N> test(s) covering <N> acceptance criteria
  Passing:  All

CI:        Passing (attempt <N> of 3)
Review:    Approved (round <N>)
Commit:    <type>(<scope>): <description>

Acceptance Criteria:
  - [x] AC-1: <criteria>
  - [x] AC-2: <criteria>
  - [x] AC-3: <criteria>

Lifecycle: Branch -> Plan -> Test -> Implement -> PR -> CI -> Review -> Merge -> Done
```

---

## Error Handling

| Error | Phase | Resolution |
|-------|-------|------------|
| Issue not found | 1 | Report error. Check issue ID. Try `linearis issues search "<title>"`. |
| No repo mapped for project | 1 | Report error. Update `linear-context.json` with the project's repo path. |
| Branch already exists | 2 | Check out existing branch, pull latest, continue from there. |
| Tests fail after 2 implementation retries | 5 | Stop. Report failure details. User must intervene. |
| Push rejected | 6 | Pull and rebase, then retry push. |
| PR creation fails | 7 | Check if PR already exists for branch. If so, use existing. Otherwise check `gh auth status`. |
| CI timeout (15 min) | 8 | Report to user. Suggest checking GitHub Actions directly. |
| CI fails after 3 fix attempts | 8 | Stop. Report failure logs. User must intervene. |
| No reviewers on PR | 9 | Inform user to add reviewers or self-approve. |
| Review rejected 5 times | 9 | Escalate. Fundamental design disagreement likely. |
| Merge conflicts | 10 | Rebase on target, resolve conflicts (subagent if complex), re-push, re-merge. |
| Merge blocked by branch protection | 10 | Report. May need admin override or additional approvals. |

---

## Subagent Summary

| Phase | Agent Type | Model | Task |
|-------|-----------|-------|------|
| 3 -- Investigate | `Architect` | `sonnet` | Read-only codebase analysis, produce TDD plan |
| 4 -- TDD Red | `general-purpose` | `sonnet` | Write failing tests |
| 5 -- Implement | `general-purpose` | `sonnet` | Write code to pass tests |
| 5 -- Retry | `general-purpose` | `sonnet` | Fix failing implementation |
| 8 -- CI Fix | `general-purpose` | `sonnet` | Read CI logs, fix failures (via MonitorCI) |
| 9 -- Review Fix | `general-purpose` | `sonnet` | Address PR feedback (via ReviewPR) |
| 10 -- Conflict | `general-purpose` | `sonnet` | Resolve merge conflicts |

---

## Validation Rules

Before any phase transition, verify the previous phase completed successfully:

| Transition | Validation |
|-----------|------------|
| Phase 1 -> 2 | Issue fetched, AC extracted, repo path resolved |
| Phase 2 -> 3 | Status is "In Progress", branch exists, pushed to remote |
| Phase 3 -> 4 | Investigation report received with test plan and implementation plan |
| Phase 4 -> 5 | Test files created, tests run and failed (red phase confirmed) |
| Phase 5 -> 6 | All tests passing, linter clean, implementation committed |
| Phase 6 -> 7 | All changes pushed to remote successfully |
| Phase 7 -> 8 | PR created, PR number and URL captured |
| Phase 8 -> 9 | CI passing |
| Phase 9 -> 10 | PR review decision is APPROVED |
| Phase 10 -> Done | PR merged, Linear status set to "Done" |

---

*Referenced by: SKILL.md | Depends on: GitWorkflow skill, IssueTemplate.md, linear-context.json, MonitorCI.md, ReviewPR.md*
