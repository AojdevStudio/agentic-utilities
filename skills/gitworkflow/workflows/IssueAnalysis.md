# IssueAnalysis Workflow

Pulls every open issue, identifies in-flight claims across worktrees, splits the backlog around a release cut (beta/mvp/v1/sprint-N), assigns issues to coding-agent worktrees, and applies a 5-label routing scheme so each agent can ask `gh issue list --label "agent:<self>"` and only see its lane.

**Default behavior:** Dry-run — emits the table and label plan but does not write. Pass `--apply` to actually create labels and edit issues.

## Variables

```
ANALYSIS_OPTIONS: $ARGUMENTS
CUT_NAME:    {{extract --cut from OPTIONS, default: "beta"}}
AGENTS:      {{extract --agents=a,b,c from OPTIONS, default: detect from `git worktree list`}}
REPO:        {{extract --repo from OPTIONS, default: derive from `git remote -v`}}
APPLY:       {{if contains OPTIONS "--apply"}}true{{else}}false{{endif}}
SKIP_PROMPT: {{if contains OPTIONS "--yes"}}true{{else}}false{{endif}}
```

## Workflow

### Phase 0: Discovery

Run in parallel — these are read-only:

1. `git worktree list` → enumerate sibling worktrees, infer agent names from path suffix (`<repo>-claude` → `claude`, `<repo>-codex` → `codex`, etc.). If only one worktree exists, ask the user for the agent list.
2. `git branch --sort=-committerdate | head -20` → recently active branches across worktrees.
3. `gh pr list --state open --json number,title,headRefName,author,createdAt` → in-flight PRs.
4. `gh issue list --state open --limit 100 --json number,title,labels,assignees,milestone,updatedAt` → full open backlog.
5. For any issue without a clear claim, fetch its last comment to detect manual claims (`gh issue view N --json comments,assignees`).

Cross-reference results: every open PR + active non-main branch is a **claim signal** that ties an in-flight issue to a worktree. Record these as locked claims; the rest of the backlog is unclaimed.

### Phase 1: Cut definition

Print the open backlog grouped by label. Ask the user (skip if `--yes`):

> What defines `<CUT_NAME>` for this repo? Paste a 1-sentence north-star check.
> Example: "Beta = I open the app, the briefing tells me what changed, and the numbers are correct."

Use the answer as the cut filter. Walk each open issue and decide cut-blocker vs deferred based on whether resolving it is a precondition for the cut sentence to be true.

If the user provides no sentence, fall back to: **anything labeled `bug` or on the critical path of an existing milestone is cut-blocker; everything else is deferred.**

### Phase 2: Categorize

Build the routing table with these columns:

| Column | Source |
|--------|--------|
| Issue # | `gh issue list` |
| Title (≤40 chars) | truncated |
| Agent | from claim signal OR file-boundary heuristic (next phase) |
| Round | 1 = in flight, 2 = ready next, 3 = soak-only |
| Label set | computed |

### Phase 3: Assign unclaimed issues to agents

For each unclaimed cut-blocker, propose an agent based on:

1. **Warm context** — if an agent recently touched files in the same directory (last 10 commits on its branch), assign there.
2. **File boundary disjointness** — if two issues touch overlapping paths, do not put them in the same round across agents (would conflict at merge).
3. **Triage history** — if an issue already has a TDD plan written by a specific agent, that agent inherits it.
4. **Load balancing** — split remaining issues evenly across agents per round.

Surface the heuristic that drove each assignment in a `(why)` column when the choice isn't obvious.

### Phase 4: Round ordering

- **Round 1**: in-flight PRs + their issues. No new work assigned to an agent until its R1 lands.
- **Round 2**: cut-blocker queue, one issue per agent, picked so file boundaries stay disjoint with R1.
- **Round 3**: bug-fix soak only. Once `<CUT_NAME>-blocker` returns 0 open, ship the cut tag.

### Phase 5: Output

Print three blocks in order:

1. **Routing table** — issue → agent → round → labels (markdown table)
2. **Route map** — ASCII Gantt-style:
   ```
          ┌─ R1 ────────┐  ┌─ R2 ──────┐  ┌─ R3 ─────────┐
   <a1>  →│ <PR/issues>│ → │ <issue>   │ → │ bug-fix only │
   <a2>  →│ <PR/issues>│ → │ <issue>   │ → │ bug-fix only │ → CUT TAG
   <a3>  →│ <PR/issues>│ → │ <issue>   │ → │ bug-fix only │
          └────────────┘  └───────────┘  └──────────────┘
   ```
3. **Per-agent cheat-line**, e.g.:
   ```
   gh issue list --repo <REPO> --label "agent:claude,<CUT_NAME>-blocker" --state open
   ```

### Phase 6: Apply (only if `APPLY=true`)

1. Create the 5 labels (idempotent — `gh label create` errors if a label exists; ignore the error):
   - `agent:<name>` for each agent (cyan/orange/purple/green rotation)
   - `<CUT_NAME>-blocker` (red `DC2626`)
   - `post-<CUT_NAME>` (gray `6B7280`)

2. Apply labels to issues. **Run as separate `gh issue edit` calls — do NOT pipe through a multi-line shell loop**, because some PreToolUse safety hooks reject heredoc-style scripts. One call per issue keeps the audit trail clean.

3. Verify: print `gh issue list --label "<CUT_NAME>-blocker" --state open --json number,title --jq length` and confirm the count matches the table.

If `APPLY=false`, print the exact `gh label create` and `gh issue edit` commands the user would need to run, so they can paste-and-go.

## Examples

**Example 1: Default (dry-run, beta cut, auto-detect agents)**
```
User: "/git-workflow --issue-analysis"
→ Detects worktrees: claude, codex, pi
→ Pulls 24 open issues, 2 open PRs
→ Asks for cut sentence; user answers
→ Prints routing table + route map + cheat-lines
→ Stops (no labels written)
```

**Example 2: Apply with custom cut name**
```
User: "/git-workflow --issue-analysis --cut mvp --apply"
→ Creates labels: agent:claude, agent:codex, agent:pi, mvp-blocker, post-mvp
→ Edits 24 issues with appropriate labels
→ Verifies counts and prints final table
```

**Example 3: Single-agent repo**
```
User: "analyze the issues for this repo"
→ Only one worktree found, so prompts: "List your agents (comma-separated):"
→ User: "human"
→ Skips agent: labels (only one assignee), still creates cut-blocker / post-cut split
```

## When to invoke this workflow

- User says: "analyze the issues", "route issues across worktrees", "label issues for the beta cut", "plan the cut", "who should work on what", "what's left for beta/mvp/v1".
- User passes flag: `--issue-analysis` to GitWorkflow.
- A new chunk of issues just landed (e.g. after `/to-issues`) and the user wants them slotted into the existing cut plan — re-run with `--apply` to fill in only the new issues.

## Gotchas

- **Multi-line shell scripts can trip safety hooks.** Apply labels with one `gh issue edit` per call. Do not chain with `&` + `wait` inside a heredoc.
- **`gh label create` is not idempotent.** It errors on duplicate names. Wrap with `|| true` or check existing labels first via `gh label list --json name --jq '.[].name'`.
- **Worktree path → agent name** assumes the convention `<repo>-<agent>`. If a worktree path doesn't match, fall back to asking the user.
- **In-flight PR ≠ owned issues automatically.** A single PR can bundle multiple issues. Read the PR body for `Closes #N` / `Fixes #N` references and tie those issues to the PR's worktree.
- **Cut name is parameterized; default is `beta`.** Don't hard-code "beta" anywhere — use the `<CUT_NAME>` template variable so the same workflow ships mvp / v1 / sprint-3 cuts unchanged.
- **Color rotation for >4 agents.** Default palette covers 4: cyan `0EA5E9`, orange `F97316`, purple `A855F7`, green `10B981`. Beyond that, the workflow asks the user for colors.
