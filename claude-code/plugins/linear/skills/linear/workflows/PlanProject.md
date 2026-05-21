# PlanProject Workflow

Decompose a project or feature into structured Linear issues with dependency ordering, estimates, and batch creation.

## Triggers

- "plan project", "break this down", "decompose feature"
- "create project plan", "plan this feature"
- "split this into issues", "break into tasks"

## Prerequisites

Before starting decomposition:
- [ ] Linear CLI (`linearis`) is available
- [ ] `LINEAR_API_TOKEN` set in environment or `~/.env.secrets`
- [ ] GraphQL helper available at `${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh`
- [ ] IssueTemplate format loaded from `${CLAUDE_PLUGIN_ROOT}/skills/linear/IssueTemplate.md`
- [ ] CreateIssue workflow loaded from `${CLAUDE_PLUGIN_ROOT}/skills/linear/workflows/CreateIssue.md`

---

## The Max-3 Rule

> **MAXIMUM 3 ACCEPTANCE CRITERIA PER ISSUE. NO EXCEPTIONS.**
>
> If a capability requires more than 3 testable outcomes, it MUST be split into multiple issues. This constraint ensures each issue fits within a single agent context window for automated resolution.

---

## The Max-7 Rule

> **MAXIMUM 7 ISSUES PER PROJECT DECOMPOSITION. NO EXCEPTIONS.**
>
> If a project requires more than 7 issues, it is an epic and must be
> split into sub-projects first. Each sub-project gets its own
> PlanProject decomposition.
>
> Minimum: 3 issues. If fewer than 3, it's likely a single issue, not a project.

---

## Vertical Slice Rule

> **EVERY ISSUE MUST BE A VERTICAL SLICE, NOT A HORIZONTAL ONE.**

A vertical slice cuts through all integration layers — UI + action/route + orchestrator/module + DB + tests — for a single user-facing capability. A horizontal slice addresses one layer across many capabilities. Horizontal slices are an anti-pattern in this workflow.

**Vertical (correct):**
1. User can view dashboard — thin read-only path, exercises every layer end-to-end
2. User can filter — adds query layer and UI state on top of the proven path
3. User can edit — adds mutations, auth checks, and validation on top of #2

**Horizontal (wrong, do not use):**
1. Design all DB schemas
2. Build all API endpoints
3. Build all UI pages
4. Write all tests

**Why this matters:** The first issue of any project must be the thinnest possible end-to-end feature. Vertical slices surface unknown-unknowns — auth plumbing gaps, schema mismatches, deployment surprises — in issue #1, not at the end when they are expensive to fix. Horizontal slices defer all integration risk to a late phase where it compounds.

**Acceptance criteria must reflect the full slice.** An AC like "schema migration runs" is horizontal. An AC like "authenticated user sees populated dashboard with live data" is vertical.

The Max-7 Rule still applies: the entire decomposition is capped at 7 vertical slices.

---

## Workflow Steps

### Step 1: Intake

Gather project scope from the user.

**If user provides free-form text:**
- Extract the core feature or project description
- Identify any mentioned constraints, timelines, or dependencies

**If user references an existing Linear project:**
- Fetch current issues to understand existing state:

```bash
linearis issues search "" --project "<project-name>" --team <TEAM> -l 50
```

**Ask clarifying questions** via AskUserQuestion if scope is unclear:

| Question | Why It Matters |
|----------|----------------|
| What is the target outcome? | Defines the "done" state for the entire project |
| Any technical constraints? | Shapes implementation approach and issue boundaries |
| Timeline or urgency? | Drives priority assignment and due date calculation |
| Which Linear project? | Routes issues correctly (infer from context first) |

Do NOT proceed to decomposition with an ambiguous scope. Clarity here prevents rework later.

### Step 2: Decompose

Use First Principles thinking to break the feature into fundamental capabilities.

**Invoke the Thinking skill:**

```
Skill("Thinking")
```

Apply this decomposition framework:

1. **What are the fundamental capabilities needed?**
   - Strip away assumptions. What must exist for this feature to work at its most basic level?
   - Each capability becomes a candidate issue.

2. **What are the dependencies between them?**
   - Which capabilities require others to exist first?
   - Map the dependency graph before structuring issues.

3. **What is the minimum viable implementation order?**
   - Foundation layers first, integration layers last.
   - Identify the critical path.

**Decomposition sizing guide:**

| Issue Size | Story Points | Characteristics |
|------------|-------------|-----------------|
| Trivial | 1 | Config change, copy update, single-file edit |
| Small | 2 | Single component, one API endpoint, one migration |
| Medium | 3 | Multiple files, one integration, moderate logic |
| Large | 5 | Cross-cutting concern, multiple integrations |
| Epic-sized | 8 | If hitting 8, consider splitting further |

Each decomposed capability becomes one Linear issue. Aim for issues in the 2-5 point range.

### Step 3: Structure Each Issue

For EACH issue identified in Step 2, compose a description using the IssueTemplate format.

**Title rules:**
- Clear and actionable
- Starts with a verb (Add, Create, Fix, Update, Remove, Configure, Implement)
- Specific enough to understand without reading the description
- Examples: "Add patient search API endpoint", "Configure OAuth callback for SSO", "Fix duplicate claim records in search results"

**Description structure** (from IssueTemplate at `${CLAUDE_PLUGIN_ROOT}/skills/linear/IssueTemplate.md`):

```markdown
## Summary
1-2 sentences: what gets accomplished and why.

## Context
- **Repository:** <local path from linear-context.json>
- **GitHub:** <org/repo from linear-context.json>
- **Relevant Files:** <comma-separated file paths, or "To be determined during investigation">
- **Related Issues:** <identifiers or "None">

## Current Behavior
What happens now. Use "N/A" for new features.

## Expected Behavior
What should happen after this issue is resolved.

## Acceptance Criteria (max 3)
- [ ] AC-1: [Testable outcome -- one sentence, binary pass/fail]
- [ ] AC-2: [Testable outcome]
- [ ] AC-3: [Testable outcome]

## Scope
**In scope:** [explicit inclusions]
**Out of scope:** [explicit exclusions]

## Additional Context
Logs, screenshots, error messages, design links.
```

Auto-populate the **Context** section for each issue by reading `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json`:
- Match the project to its entry and fill Repository + GitHub from `repoPath` and `github` fields
- For consulting projects (`repoPath: null`), use `Repository: N/A (consulting project)`
- Set Relevant Files based on decomposition analysis; use "To be determined" if not yet known
- Set Related Issues by cross-referencing other issues in the same decomposition

**Field assignment:**

| Field | How to Determine |
|-------|-----------------|
| Priority | 1=Urgent (blocks everything), 2=High (critical path), 3=Normal (standard), 4=Low (nice-to-have) |
| Estimate | Story points: 1, 2, 3, 5, or 8 (see sizing guide in Step 2) |
| Labels | Infer from content using label inference table below |
| Assignee | Always `<YOUR_LINEAR_USER_UUID>` |
| Team | The `defaultTeamKey` from `linear-context.json` |
| Project | Inferred from context or asked via AskUserQuestion |

**Label inference:**

| Content Pattern | Label(s) |
|----------------|----------|
| Bug reports, errors, broken, crash | Bug |
| New capabilities, new feature | Feature |
| Optimization, refactoring, cleanup | Improvement |
| Client deliverables, reports | Client-Facing + Deliverable |
| Training materials, onboarding | Training |
| HR, payroll, benefits | HR/Compensation |
| SOP, workflow, process | Operations |
| Milestone, checkpoint | Milestone |

> **REMINDER:** If any issue needs more than 3 acceptance criteria, STOP and split it into multiple issues. Each AC must be binary testable -- "works correctly" is not testable; "returns 200 with JSON body containing userId field" is testable.

### Step 4: Dependencies

Map blocking relationships between the decomposed issues.

**Ordering principles:**

| Phase | Issue Type | Examples |
|-------|-----------|----------|
| 1 - Foundation | Setup, config, infrastructure | DB migrations, env config, scaffolding |
| 2 - Core | Primary feature logic | API endpoints, business logic, core UI |
| 3 - Integration | Connecting pieces | Webhook handlers, service integrations, E2E flows |
| 4 - Polish | Quality and UX refinement | Error handling, loading states, edge cases |

**Identify blockers:**
- Issue B depends on Issue A's database schema? A blocks B.
- Issue C needs Issue A's API endpoint? A blocks C.
- Issues D and E are independent? They can run in parallel.

Note: Linear supports sub-issues (parent/child) and blocking relationships. Use blocking relationships for hard dependencies. Use sub-issues only when issues share a clear parent epic.

**Setting blocking relationships after issue creation:**

`linearis issues update` does not have a `--blocked-by` flag. Use the Linear GraphQL API directly via `linear-graphql.sh` or a raw curl call:

```bash
# Declare that <blocked-uuid> is blocked by <blocker-uuid>
curl -s -X POST https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: $LINEAR_API_TOKEN" \
  -d '{
    "query": "mutation { issueRelationCreate(input: { issueId: \"<blocked-uuid>\", relatedIssueId: \"<blocker-uuid>\", type: blocked }) { success issueRelation { id type } } }"
  }'
```

Run this for every hard dependency identified above. Explicit blocking relationships in Linear are non-negotiable — implicit ordering noted only in narrative is insufficient and will be missed by automated resolvers.

> **TODO:** Add a `set-blocking` command to `linear-graphql.sh` that wraps this mutation for convenience. Until then, use the raw curl form above.

### Step 5: Review

Present the full plan to the user BEFORE creating anything in Linear.

**Present this table:**

```
PROJECT PLAN: [Project Name]

| # | Title | Priority | Estimate | Dependencies | Labels | Key Files |
|---|-------|----------|----------|--------------|--------|-----------|
| 1 | [Issue title] | P2 | 3 pts | None | Feature | src/db/schema.ts |
| 2 | [Issue title] | P3 | 2 pts | Blocked by #1 | Feature | src/api/endpoint.ts |
| 3 | [Issue title] | P3 | 5 pts | Blocked by #1 | Feature | src/components/Widget.tsx |
| 4 | [Issue title] | P3 | 3 pts | Blocked by #2, #3 | Improvement | src/services/integration.ts |
| 5 | [Issue title] | P4 | 2 pts | Blocked by #4 | Improvement | src/utils/validation-helper.ts |

TOTALS
- Issues: 5 (within Max-7 Rule)
- Estimated effort: 15 story points
- Critical path: #1 -> #2 -> #4 -> #5

EXECUTION PLAN
==============
Day 1: #1 (foundation, unblocks #2 and #3)
Day 2: #2 + #3 (parallel, independent)
Day 3: #4 (integration, requires #2 + #3)
Day 4: #5 (polish, final layer)

Critical path: #1 -> #2 -> #4 -> #5
Parallel opportunities: #2 and #3 after #1 completes
```

**Wait for user approval:**

Use AskUserQuestion:
```
Ready to create these [N] issues in Linear? You can:
- Approve all
- Remove specific issues by number
- Adjust priorities or estimates
- Add/modify issues before creation
```

Do NOT proceed to Step 5.5 without explicit user approval.

### Step 5.5: Parallelization Analysis & Consolidation Offer

After the user approves the plan, analyze whether issues can be executed in parallel or must be sequential, and offer to consolidate sequential groups into single issues for efficiency.

**Why this matters:** When issues share mutable state (same files), they cannot be worked on in parallel — an agent resolving one would conflict with an agent resolving another. Consolidating sequential groups into a single issue eliminates unnecessary context-switching overhead and creates cleaner branches.

#### 5.5a: Classify each issue's file boundaries

For each issue, extract the primary files it modifies (from the "Key Files" or "Relevant Files" field). Build a file-overlap matrix:

```
File Overlap Matrix:
  #1 globals.css
  #2 EmailGate.tsx
  #3 Explorer.tsx
  #4 Explorer.tsx
  #5 Explorer.tsx
  #6 page.tsx

Overlap groups:
  Group A: #1 (globals.css) — no overlap
  Group B: #2 (EmailGate.tsx) — no overlap
  Group C: #3, #4, #5 (Explorer.tsx) — shared file, must be sequential
  Group D: #6 (page.tsx) — no overlap, but check if any later issue also touches page.tsx
```

#### 5.5b: Apply sub-agent routing rules

Classify using the following boolean conditions:

**PARALLEL — ALL must be true:**
1. Tasks do NOT share mutable state (no overlapping files)
2. Each task has clear file boundaries (distinct files/directories)
3. No task's output is another task's input
4. No ordering dependency affects correctness

**SEQUENTIAL — ANY triggers sequential:**
1. Tasks modify the same file
2. A later task depends on an earlier task's output
3. Ordering affects correctness (e.g., migration before seed)

#### 5.5c: Present the parallelization analysis

```
PARALLELIZATION ANALYSIS
========================

Parallel group (can run simultaneously):
  • #1 Global CSS fixes (globals.css)           1pt
  • #2 EmailGate a11y (EmailGate.tsx)           2pt
  • #6 Page link focus (page.tsx)               1pt

Sequential group — shared file: Explorer.tsx
  • #3 Staff table aria-labels                  1pt
  • #4 Slider label association                 2pt
  • #5 Focus-visible on buttons                 2pt
  → Total: 5pts, must be sequential (same file)

Execution: 3 parallel + 3 sequential = could be 3 parallel + 1 combined
```

#### 5.5d: Offer consolidation

Use AskUserQuestion to ask:

```
The sequential group (#3, #4, #5) all modify the same file and must be done in order.
Would you like to:

1. Keep as separate issues (3 issues, resolved one at a time)
2. Combine into a single issue (1 issue, all changes in one branch)
   → Combined estimate: [sum]pts, description references all original tasks
```

**If user chooses "combine":**
- Create one combined issue with:
  - Title: `[SharedFile]: [summary of all grouped tasks]`
  - Description listing each original task as a section
  - Estimate = sum of original estimates
  - Priority = highest priority in the group
  - Labels = union of all labels in the group
- Remove the individual issues from the creation queue
- Update the plan table and execution order before proceeding to Step 6

**If user chooses "keep separate":**
- Proceed as-is. Note in the execution plan that these must be resolved sequentially.

#### 5.5e: Edge cases

| Scenario | Handling |
|----------|----------|
| All issues touch different files | Skip consolidation — just note "all issues can run in parallel" |
| All issues touch the same file | Offer to combine all into one issue |
| Multiple sequential groups exist | Offer consolidation for each group independently |
| A file is touched by issues in different phases | Sequential within that file, but may parallel with other files |
| Issue touches 2+ files, one shared | Include in sequential group for the shared file |

### Step 6: Batch Create

For EACH approved issue, execute the two-step creation process.

**Step 6a: Create via linearis CLI**

```bash
linearis issues create "<title>" \
  -d "<formatted description from Step 3>" \
  -a <YOUR_LINEAR_USER_UUID> \
  -p <priority 1-4> \
  --team <TEAM> \
  --project "<project-name>" \
  --labels "<inferred labels>"
```

Parse the JSON output to extract the issue UUID (`id` field) and identifier (e.g., `<TEAM>-170`).

**Step 6b: Set due date and estimate via GraphQL helper**

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date-and-estimate "<issue UUID>" "YYYY-MM-DD" <story-points>
```

If no due date was provided for a specific issue, calculate based on:
- Issue position in dependency chain
- Estimated complexity (1 pt ~ 1 day, 2 pts ~ 2 days, 3 pts ~ 3 days, 5 pts ~ 1 week, 8 pts ~ 1.5 weeks)
- Buffer between dependent phases

**Track all created issues:**

Maintain a running list of:
- Issue identifier (e.g., <TEAM>-170)
- Issue UUID (for GraphQL operations)
- Title
- Creation status (success/failed)

**Error recovery during batch creation:**
- If an issue fails to create, log the error and continue with remaining issues
- After batch completes, report which issues failed and offer to retry

### Step 6.5: Auto-Create Milestones

After all issues are created, group them by their dependency phase (from Step 4) and create a milestone for each phase.

**Milestone creation requires a project ID.** If the issues are being added to an existing project, use that project's ID. If this is a new decomposition without a project, skip milestone creation and note it in the report.

```
For each phase in the dependency map:
1. Determine phase name from the ordering principles table in Step 4:
   - Phase 1: Foundation (setup, config, infrastructure)
   - Phase 2: Core Implementation (primary feature logic)
   - Phase 3: Integration (connecting pieces)
   - Phase 4: Polish & Launch (quality and UX refinement)

2. Calculate target date:
   - Phase 1 target = project start date + sum of Phase 1 issue estimates (in days)
   - Phase 2 target = Phase 1 target + 1 buffer day + sum of Phase 2 estimates
   - Phase 3 target = Phase 2 target + 1 buffer day + sum of Phase 3 estimates
   - Phase 4 target = Phase 3 target + 1 buffer day + sum of Phase 4 estimates
   - Point-to-day conversion: 1pt=1d, 2pt=2d, 3pt=3d, 5pt=5d (1wk), 8pt=8d (1.5wk)

3. Create milestone:
   ${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh create-milestone \
     "<projectId>" "Phase N: <phase-name>" "<targetDate>" \
     "Issues: <comma-separated issue identifiers>"
```

**Track created milestones** alongside the issue tracking list:
- Milestone name
- Target date
- Issues included
- Creation status (success/failed)

If milestone creation fails, log the error but do NOT fail the entire workflow. Issues are the primary deliverable; milestones are supplementary.

See `Workflows/ManageMilestones.md` for full milestone naming conventions and operations.

### Step 7: Report

Output the final summary after all issues are created.

**Summary table:**

```
PROJECT CREATED: [Project Name]

| ID | Title | Priority | Estimate | Status |
|----|-------|----------|----------|--------|
| <TEAM>-170 | [Title] | P2 | 3 pts | Created |
| <TEAM>-171 | [Title] | P3 | 2 pts | Created |
| <TEAM>-172 | [Title] | P3 | 5 pts | Created |
| <TEAM>-173 | [Title] | P3 | 3 pts | Created |
| <TEAM>-174 | [Title] | P4 | 2 pts | Created |

MILESTONES
| # | Milestone | Target Date | Issues | Points |
|---|-----------|-------------|--------|--------|
| 1 | Phase 1: Foundation | 2026-03-07 | <TEAM>-170 | 3 pts |
| 2 | Phase 2: Core Implementation | 2026-03-12 | <TEAM>-171, <TEAM>-172 | 7 pts |
| 3 | Phase 3: Integration | 2026-03-16 | <TEAM>-173 | 3 pts |
| 4 | Phase 4: Polish & Launch | 2026-03-18 | <TEAM>-174 | 2 pts |

SUMMARY
- Total issues: 5
- Total effort: 15 story points
- Milestones: 4 (auto-created)
- All assigned to: [Your name]
- Project: [Project Name]
- Team: <TEAM>

EXECUTION ORDER
Parallel:  <TEAM>-170 + <TEAM>-171 (independent files)
Then:      <TEAM>-172 → <TEAM>-173 → <TEAM>-174 (shared file, sequential)

[If consolidation was applied in Step 5.5:]
CONSOLIDATION APPLIED
- <TEAM>-172, <TEAM>-173, <TEAM>-174 combined into <TEAM>-175 (13pts, Explorer.tsx)
- Original issues canceled

View in Linear: https://linear.app/<workspace>/project/[project-slug]
```

---

## Error Handling

| Error | Resolution |
|-------|------------|
| `linearis` CLI not found | Prompt user to install: `bun install -g linearis` |
| Team not found | Read the `defaultTeamKey` from `linear-context.json` |
| Project not found | Ask user to confirm project name via AskUserQuestion |
| GraphQL helper fails | Check `LINEAR_API_TOKEN` in `~/.env.secrets` |
| Issue creation fails mid-batch | Log failure, continue remaining, report at end |
| User rejects plan in review | Return to Step 2 with user feedback, re-decompose |
| Scope too vague after intake | Ask additional clarifying questions, do not guess |
| Issue exceeds 3 AC | Split into multiple issues before presenting plan |
| Estimate exceeds 8 points | Split the issue -- it is too large for a single unit of work |
| Decomposition exceeds 7 issues | Split into sub-projects per Max-7 Rule; each sub-project gets its own PlanProject |
| Decomposition has fewer than 3 issues | Likely a single issue, not a project -- use CreateIssue instead |

---

## Output Format

Every PlanProject execution produces output in this structure:

```
PLAN PROJECT: [Name]

INTAKE
- Scope: [1-2 sentence summary]
- Constraints: [Any noted constraints]
- Timeline: [Target timeline]

DECOMPOSITION
- [N] capabilities identified
- [N] issues structured
- [X] total story points

ISSUES
[Summary table from Step 7]

EXECUTION ORDER
[Numbered sequence with parallel groups noted]

LINK
[Linear project URL]
```

---

*Referenced by: SKILL.md (workflow routing table)*
*References: IssueTemplate.md, CreateIssue.md, Thinking skill*
