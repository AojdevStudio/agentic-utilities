# ManageMilestones Workflow

Create and manage project milestones in Linear for phase-based delivery tracking.

## Triggers

- "create milestone", "add milestone", "project milestone"
- "set milestone for project"

## Prerequisites

- [ ] `LINEAR_API_TOKEN` set in environment or `~/.env.secrets`
- [ ] GraphQL helper available at `${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh`
- [ ] Target project ID known (from `linear-context.json` or user input)

---

## Operations

### Create Milestone

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh create-milestone \
  "<projectId>" "<name>" "<targetDate>" "<description>"
```

**Fields:**

| Field | Required | Action |
|-------|----------|--------|
| projectId | YES | The project this milestone belongs to. Resolve from `linear-context.json` or ask user. |
| name | YES | Descriptive phase name (see naming convention below) |
| targetDate | YES | When this milestone should be complete (YYYY-MM-DD) |
| description | RECOMMENDED | Markdown description of milestone scope and included issues |

### Milestone Naming Convention

Use phase-based names that communicate delivery stages:

| Phase | Name Pattern | Typical Content |
|-------|-------------|-----------------|
| 1 | Phase 1: Foundation | Setup, config, infrastructure, DB migrations |
| 2 | Phase 2: Core Implementation | Primary feature logic, API endpoints, business rules |
| 3 | Phase 3: Integration | Service connections, E2E flows, webhook handlers |
| 4 | Phase 4: Polish & Launch | Error handling, UX refinement, edge cases, deploy |

For projects with custom phases, adapt the naming while keeping the "Phase N: Description" format.

**Examples:**
- "Phase 1: Database Schema & API Scaffold"
- "Phase 2: Search & Filtering"
- "Phase 3: Third-Party Integration"
- "Phase 4: QA & Production Deploy"

### Workflow

1. Resolve project ID from name or context
2. Determine milestone name (follow naming convention)
3. Calculate or confirm target date
4. Compose description listing issues included in the milestone
5. Create milestone via GraphQL helper
6. Report milestone details

### Target Date Calculation

When calculating milestone dates from issue estimates:

| Story Points | Approximate Duration |
|-------------|---------------------|
| 1 pt | 1 day |
| 2 pts | 2 days |
| 3 pts | 3 days |
| 5 pts | 1 week |
| 8 pts | 1.5 weeks |

**Formula:**
- Phase N target = Previous phase target + sum of Phase N issue estimates (converted to days)
- Add 1 buffer day between phases for review and handoff
- Phase 1 starts from the project start date (today if not specified)

---

## Output Format

```
MILESTONE CREATED
=================
Project: <project name> (<projectId>)
Milestone: <name>
Target Date: YYYY-MM-DD
Description: <scope summary>

Issues Included:
- <TEAM>-170: <title> (3 pts)
- <TEAM>-171: <title> (2 pts)
Phase Total: [N] story points
```

When creating multiple milestones:

```
MILESTONES CREATED: [Project Name]

| # | Milestone | Target Date | Issues | Points |
|---|-----------|-------------|--------|--------|
| 1 | Phase 1: Foundation | 2026-03-07 | <TEAM>-170, <TEAM>-171 | 5 pts |
| 2 | Phase 2: Core Implementation | 2026-03-12 | <TEAM>-172, <TEAM>-173 | 8 pts |
| 3 | Phase 3: Integration | 2026-03-17 | <TEAM>-174 | 3 pts |
| 4 | Phase 4: Polish & Launch | 2026-03-19 | <TEAM>-175 | 2 pts |

Total: [N] milestones, [N] story points, target completion: YYYY-MM-DD
```

---

## Error Handling

| Error | Resolution |
|-------|------------|
| GraphQL helper fails | Check `LINEAR_API_TOKEN` in `~/.env.secrets` |
| Project not found | Search Linear for project name, ask user to confirm |
| Invalid date format | Ensure YYYY-MM-DD format |
| No project ID available | Ask user which project via AskUserQuestion |

---

*Referenced by: SKILL.md (workflow routing table), PlanProject.md (Step 6.5)*
*References: linear-graphql.sh, linear-context.json*
