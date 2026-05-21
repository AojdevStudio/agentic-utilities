# ShipDaily Workflow

Daily shipping orchestration: review backlog, prioritize, select issues, resolve them, and report.

## Triggers

- "ship daily", "ship", "daily ship"
- "what should I work on", "daily standup"
- "what's next", "pick up work"

## Prerequisites

- [ ] `linearis` CLI installed and authenticated
- [ ] `LINEAR_API_TOKEN` set in environment or `~/.env.secrets`
- [ ] `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json` accessible (for project-to-repo mappings)
- [ ] `${CLAUDE_PLUGIN_ROOT}/skills/linear/workflows/ResolveIssue.md` accessible (for execution phase)

---

## Step 1: Review -- Fetch Open Issues

Fetch all open issues assigned to the user across your team. Read the `defaultTeamKey` from `linear-context.json`.

```bash
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  --team <TEAM> \
  --status "Todo,In Progress" \
  -l 25
```

Also fetch issues awaiting review:

```bash
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  --team <TEAM> \
  --status "In Review" \
  -l 25
```

Parse the JSON output from both queries. For each issue, extract:
- `identifier` (e.g., <TEAM>-123)
- `title`
- `priority` (1=Urgent, 2=High, 3=Normal, 4=Low)
- `dueDate` (ISO date or null)
- `estimate` (story points or null)
- `status` ("Todo", "In Progress", or "In Review")
- `project` (project name)
- `labels` (array)

Separate into three groups:

**Awaiting Review**: Issues with status "In Review" that have open PRs. Check PR status for each (CI passing/failing, review approved/changes requested/pending).

**In Progress**: Issues with status "In Progress" with active branches. Check if existing branches exist in the corresponding repo.

**Todo**: Issues with status "Todo". These are candidates to pick up today.

---

## Step 2: Prioritize

### Sorting Algorithm

Sort the **Todo** issues by these criteria in order of precedence:

1. **Priority** (ascending: 1=Urgent first, 4=Low last)
2. **Due date** (ascending: soonest first; null due dates sort to bottom)
3. **Estimate** (ascending: smallest first -- ship quick wins early)

### Overdue Detection

Compare each issue's `dueDate` against today's date. Flag overdue issues:
- If `dueDate < today` --> mark as **OVERDUE**
- Overdue issues sort to the absolute top regardless of priority

### Filter Out Non-Code Projects

Read `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json` and check the `repoPath` field for each issue's project:

- If `repoPath` is `null` --> the project is consulting/non-code (no code repository)
- **Exclude** these from the candidate list for automated resolution
- Still **show** them in the awareness section so the user knows they exist

### Final Priority Order

```
1. Overdue issues (any priority)
2. P1 Urgent issues
3. P2 High issues (sorted by due date, then estimate)
4. P3 Normal issues (sorted by due date, then estimate)
5. P4 Low issues (sorted by due date, then estimate)
```

---

## Step 3: Recommend

### Present Top 3 Candidates

Show the top 3 code-eligible issues in a decision table:

```
DAILY SHIP CANDIDATES

| # | Issue | Project | Priority | Due | Estimate | Status |
|---|-------|---------|----------|-----|----------|--------|
| 1 | <TEAM>-123: Fix search | Project A | P1 Urgent | OVERDUE (Feb 28) | 2 pts | Todo |
| 2 | <TEAM>-456: Add export | Project B | P2 High | Mar 5 | 3 pts | Todo |
| 3 | <TEAM>-789: Refactor auth | Project C | P3 Normal | Mar 10 | 1 pt | Todo |
```

### Show Awaiting Review Issues (Check These First)

Above the candidates, list any Awaiting Review issues. These are closest to done:

```
AWAITING REVIEW (check these first)

| Issue | PR | CI | Review Status | Action Needed |
|-------|----|----|---------------|---------------|
| <TEAM>-555: Auth refactor | #45 | pass | Changes Requested | Address feedback |
| <TEAM>-666: Add export | #46 | pass | Pending | Waiting for reviewer |
```

### Show In Progress Issues

Below the Awaiting Review section, list any In Progress issues for awareness:

```
CURRENTLY IN PROGRESS

| Issue | Project | Priority | Due | Branch |
|-------|---------|----------|-----|--------|
| <TEAM>-777: Search fix | Project A | P2 High | Mar 4 | feature/<team>-777-search-fix |
```

### Show Filtered Consulting Issues

If any non-code issues were filtered out, mention them:

```
CONSULTING (non-code, excluded from automated resolution)
- <TEAM>-333: Review onboarding playbook (Project D) -- P2, Due Mar 3
```

### User Selection

Use **AskUserQuestion** to let the user choose what to work on:

- **Option 1**: First candidate (e.g., "<TEAM>-123: Fix search")
- **Option 2**: Second candidate (e.g., "<TEAM>-456: Add export")
- **Option 3**: Third candidate (e.g., "<TEAM>-789: Refactor auth")
- **Option 4**: "Continue in-progress issue" (if any exist)
- **Option 5**: "Other" (user specifies a different issue)

Allow multi-select: the user can pick more than one issue to resolve in a session (e.g., "1 and 3").

---

## Step 4: Execute

For each selected issue, follow the **ResolveIssue** workflow end-to-end.

### Execution Priority Order

Work on issues in this order of priority:

1. **Awaiting Review with Changes Requested** -- closest to done. Re-enter ResolveIssue at Phase 9 (address review feedback).
2. **In Progress issues** -- continue where left off. Determine phase based on branch/PR state.
3. **New Todo issues** -- start fresh with ResolveIssue Phase 1.

### Phase-Aware Re-Entry Logic

Not every issue starts from Phase 1. Determine the correct entry point:

| Issue State | Re-Entry Phase | Rationale |
|-------------|---------------|-----------|
| Awaiting Review + Changes Requested | Phase 9 (Address review feedback) | PR exists, CI passed, just needs fixes |
| Awaiting Review + Pending review | Skip -- nothing to do | Wait for reviewer |
| In Progress + has PR | Phase 8 (Monitor CI) | PR exists, check CI then wait for review |
| In Progress + has branch, no PR | Phase 6 (Commit and push) | Code exists, needs PR creation |
| In Progress + no branch | Phase 2 (Create branch) | Work was started in Linear but not in code |
| Todo | Phase 1 (Fetch issue details) | Fresh start |

### Execution Protocol

1. **Read** the ResolveIssue workflow:
   ```
   Read file: ${CLAUDE_PLUGIN_ROOT}/skills/linear/workflows/ResolveIssue.md
   ```

2. **Follow** it sequentially for each issue, entering at the appropriate phase based on the re-entry logic above:
   - Phase 1: Fetch issue details from Linear
   - Phase 2: Create feature branch in the project repo
   - Phase 3: Investigate codebase and plan approach
   - Phase 4: Write tests first (TDD)
   - Phase 5: Implement to make tests pass
   - Phase 6: Commit with conventional format
   - Phase 7: Create PR
   - Phase 8: Monitor CI
   - Phase 9: Handle PR review feedback
   - Phase 10: Merge and close

3. **Sequential execution**: If resolving multiple issues, do them ONE AT A TIME. Each issue deserves full attention through all phases. Do not parallelize.

4. **Track results** for each resolved issue:

   | Field | Value |
   |-------|-------|
   | Issue ID | <TEAM>-123 |
   | Issue Title | Fix search |
   | PR URL | github.com/<org>/<repo>/pull/45 |
   | Tests Written | 3 |
   | Tests Passing | 3/3 |
   | CI Status | Passing / Failing / Pending |
   | Linear Status | In Review / Done |

### Handling Blockers

If an issue cannot be fully resolved during execution:

- Document what was accomplished and what remains
- Leave the issue in "In Progress" status
- Record the blocker reason for the daily report
- Move on to the next selected issue

---

## Step 5: Report -- Daily Summary

After all selected issues have been worked on, produce the daily shipping report.

### Report Template

```
## Daily Ship Report -- [YYYY-MM-DD]

### Resolved (Merged)
| Issue | PR | Tests | CI | Review | Merged | Linear |
|-------|----|-------|----|--------|--------|--------|
| <TEAM>-123: Fix search | #45 | 3/3 | pass | Approved | Yes | Done |

### In Review (Awaiting Merge)
| Issue | PR | Tests | CI | Review | Linear |
|-------|----|-------|----|--------|--------|
| <TEAM>-789: Refactor auth | #46 | 5/5 | pass | Pending | In Review |
| <TEAM>-456: Add export | #47 | 2/2 | pass | Changes Requested | In Review |

### Still In Progress
| Issue | Phase | Blocker |
|-------|-------|---------|
| <TEAM>-333: Update schema | Phase 5 | Test failure -- needs upstream fix |

### Remaining Backlog
- X Todo issues, Y In Progress, Z In Review
- Next priority: <TEAM>-NNN ([priority], due [date])
- Overdue count: N issues

### Session Stats
- Issues resolved (merged): N
- Issues moved to review: N
- PRs created: N
- Tests written: N
- Total shipping time: ~Nh Nm
```

### Wrap-Up Prompt

After presenting the report, use **AskUserQuestion** to offer next steps:

- **Option 1**: "Pick up next issue" (restart from Step 3 with remaining backlog)
- **Option 2**: "Wrap up for today" (end workflow)
- **Option 3**: "Review a specific issue" (user names one)

---

## Output Format

The complete workflow output follows this structure:

```
DAILY SHIP -- [DATE]

[Step 1 output: issue counts and groupings]
[Step 2 output: sorted candidate list]
[Step 3 output: recommendation table + user prompt]

--- After user selection ---

[Step 4 output: per-issue ResolveIssue execution logs]

--- After execution ---

[Step 5 output: daily report]
```

---

## Error Handling

| Error | Resolution |
|-------|------------|
| `linearis` CLI not found | Prompt user to install: `bun install -g linearis` |
| No issues returned | Confirm assignee UUID and team key are correct; try broader search without status filter |
| `linear-context.json` missing | Cannot determine repo paths; ask user for project-to-repo mapping |
| ResolveIssue workflow missing | Cannot execute resolution; fall back to manual issue work |
| API rate limit | Wait 60 seconds and retry; if persistent, reduce `-l` limit |
| Branch already exists | Issue may have prior work; check branch status before creating new one |
| Test failures during resolve | Document in report as blocker; do not force-merge |
| No code-eligible issues | All issues are consulting/non-code; inform user and show the full list for manual triage |

---

## Notes

- **Consulting projects are filtered out** from automated resolution because they have `repoPath: null` in `linear-context.json`. They still appear in the awareness section so nothing is hidden.
- **The sorting algorithm is deterministic.** Given the same set of issues, it always produces the same order. Overdue > Priority > Due date > Estimate.
- **Multi-issue sessions are sequential.** Context-switching between repos mid-issue causes mistakes. Finish one issue completely before starting the next.
- **The ResolveIssue workflow is the single source of truth** for how an issue gets resolved. ShipDaily orchestrates *which* issues to resolve and *when*, but delegates the *how* entirely to ResolveIssue.
