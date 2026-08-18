# Issue Analysis Workflow

Pull the open backlog, detect in-flight claims across worktrees, split the backlog around a release cut, assign issues to agents, and apply label-lane routing so each agent sees only its lane via `gh issue list --label "agent:<self>"`.

**Default: dry-run** — emit the table and label plan, write nothing. `--apply` creates labels and edits issues. Other flags: `--cut <name>` (default `beta`), `--agents a,b,c` (default: detect from `git worktree list`), `--yes` (skip prompts).

## Discovery (read-only, parallel)

Worktrees (agent names from the `<repo>-<agent>` path convention — non-matching paths → ask), recent branches, open PRs, the full open-issue backlog, and last comments on unclaimed issues. Every open PR + active branch is a **claim signal** tying an issue to a worktree; a single PR can bundle several issues, so read its body's `Closes #N` references. Claimed issues are locked; the rest is the unclaimed pool.

## Cut definition

Ask (unless `--yes`) for a one-sentence north-star: *"Beta = I open the app, it tells me what changed, and the numbers are correct."* An issue is a cut-blocker iff resolving it is a precondition for that sentence. No sentence given → fallback: `bug`-labeled or on an existing milestone's critical path = blocker, rest deferred.

## Assignment heuristics (unclaimed blockers)

1. **Warm context** — the agent whose recent commits touch the same directories.
2. **File-boundary disjointness** — issues with overlapping paths never run in the same round across agents (merge conflicts).
3. **Triage inheritance** — an issue with an agent-authored plan stays with that agent.
4. **Load balance** the remainder. Surface the deciding heuristic in a `(why)` column when non-obvious.

## Rounds

R1 = in-flight PRs and their issues; no new work for an agent until its R1 lands. R2 = one blocker per agent, file-disjoint with R1. R3 = bug-fix soak only; when the blocker label count hits zero, ship the cut tag.

## Output (three blocks)

1. Routing table: issue → agent → round → labels.
2. ASCII route map (R1 → R2 → R3 lanes per agent, CUT TAG at the end).
3. Per-agent cheat-line: `gh issue list --repo <R> --label "agent:<self>,<cut>-blocker" --state open`.

## Apply (`--apply` only)

Create labels — `agent:<name>` per *detected* agent only (never invent names), `<cut>-blocker` red `DC2626`, `post-<cut>` gray `6B7280`; palette rotation for agents: `0EA5E9`, `F97316`, `A855F7`, `10B981`, ask beyond four. Then label the issues and verify the blocker count matches the table. Dry-run instead prints the exact commands to paste.

If the repo uses a Projects v2 board, resolve owner and project number at runtime and add issues to it; unresolvable → `Project: none resolved`, skip. Never guess.

## Gotchas (verified)

- **One `gh issue edit` per call** — multi-line shell loops and heredocs trip PreToolUse safety hooks, and per-issue calls keep the audit trail clean.
- **`gh label create` is not idempotent** — it errors on duplicates; `|| true` or check `gh label list` first.
