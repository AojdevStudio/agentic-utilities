# CreateProject Workflow

Create a new Linear project with a compelling overview, structured content, and optional issue/initiative/milestone linking.

## Triggers

- "create project", "new project", "add project", "start a project"
- "set up a project for X", "make a project"

## Prerequisites

Before creating a project, ensure:
- [ ] `LINEAR_API_TOKEN` set in environment or `~/.env.secrets`
- [ ] GraphQL helper available at `${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh`
- [ ] `linearis` CLI is available (for issue linking)

---

## Project Creation Checklist (MANDATORY)

**ALL fields must be populated. No partial projects.**

| Field | Required | Default | Action |
|-------|----------|---------|--------|
| `name` | YES | -- | From user input |
| `description` | YES | -- | Short summary, MAX 255 chars |
| `content` | YES | -- | Full markdown project overview (unlimited) |
| `priority` | YES | 3 | 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low. ASK via AskUserQuestion if not clear from context |
| `startDate` | YES | today | YYYY-MM-DD format |
| `targetDate` | YES | -- | ALWAYS prompt user if not provided |
| `leadId` | YES | `<YOUR_LINEAR_USER_UUID>` | Project lead |
| `teamIds` | YES | `<YOUR_LINEAR_TEAM_UUID>` | Owning team |
| `state` | YES | `started` | planned, started, paused, completed, canceled |
| `initiative` | RECOMMENDED | -- | Link to existing initiative if relevant |

---

## Workflow Steps

### Step 1: Intake

Parse user request for project scope.

- Extract the project name, purpose, and any mentioned constraints or timelines
- Read the `defaultTeamKey` and lead from `linear-context.json`
- Check if there is an existing initiative to link to
- Look up the project in `linear-context.json` for repo path, GitHub URL, and tech context

### Step 2: Gather Details

Use AskUserQuestion for any missing required fields. Specifically:

| Question | When to Ask |
|----------|-------------|
| Priority (1-4)? | If not clear from context |
| Target date? | ALWAYS ask if not provided |
| Link existing issues? | If user mentions related work |
| State (planned/started)? | If ambiguous -- default to `started` |

### Step 3: Compose Content

Compose two distinct text fields:

**`description`** (max 255 chars): Write a concise 1-2 sentence summary. This appears in list views and cards. Focus on the single most important outcome.

> Example: "RAG-based document narrative generator with domain intelligence. Next.js + FastAPI + Qdrant."

**`content`** (unlimited markdown): Write a comprehensive project overview using the template below. This is the project's single source of truth -- do NOT create a separate "Project Brief" document.

---

#### Project Content Template

```markdown
## Problem Statement

[1-3 sentences. What pain point or gap does this project address? Why does it matter now? Ground it in a real user need or business constraint. Avoid vague statements like "improve efficiency" -- name the specific friction.]

## Vision

[2-4 sentences. What does the world look like when this project ships? Paint the end state. What can the user or business do that they cannot do today? This is aspirational but concrete.]

## Scope

**In scope:**
- [Explicit capability or deliverable 1]
- [Explicit capability or deliverable 2]
- [Explicit capability or deliverable 3]

**Out of scope:**
- [Explicit exclusion 1 -- prevents scope creep]
- [Explicit exclusion 2]

## Architecture & Tech Stack

[If applicable. Name the frameworks, services, infrastructure, and key libraries. Include the repo path and GitHub URL from linear-context.json. For consulting projects, describe the deliverable format instead (e.g., Google Docs, Sheets).]

| Layer | Technology |
|-------|-----------|
| Frontend | [e.g., Next.js 15, Tailwind CSS] |
| Backend | [e.g., FastAPI, Supabase Edge Functions] |
| Database | [e.g., Supabase Postgres, Qdrant] |
| Infrastructure | [e.g., Vercel, Docker, GitHub Actions] |

**Repository:** [local path from linear-context.json]
**GitHub:** [org/repo from linear-context.json]

## Key Milestones

| # | Milestone | Target | Description |
|---|-----------|--------|-------------|
| 1 | Foundation | [date] | [What gets built in this phase] |
| 2 | Core Implementation | [date] | [Primary feature logic] |
| 3 | Integration & Testing | [date] | [Connecting pieces, E2E tests] |
| 4 | Launch & Polish | [date] | [Final QA, deployment, docs] |

## Success Criteria

- [ ] [Measurable outcome 1 -- e.g., "All 5 API endpoints return correct responses with <200ms p95 latency"]
- [ ] [Measurable outcome 2 -- e.g., "Dashboard loads provider KPIs for 3 test practices"]
- [ ] [Measurable outcome 3 -- e.g., "Deployed to production with CI/CD pipeline passing"]

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| [What could go wrong] | [High/Medium/Low] | [How to prevent or respond] |
| [Second risk] | [Impact] | [Mitigation] |

## Dependencies

- **Upstream:** [What must exist before this project can start -- APIs, credentials, approvals]
- **Downstream:** [What depends on this project shipping -- other projects, client commitments]

## Key Decisions

[Track architectural or business decisions made during planning. Update this section as the project evolves.]

| Decision | Rationale | Date |
|----------|-----------|------|
| [e.g., "Use Supabase over Firebase"] | [e.g., "Postgres flexibility, Row Level Security, existing team familiarity"] | [date] |
```

---

#### Content Composition Guidelines

1. **Problem Statement first.** Every project exists to solve a problem. If you cannot articulate the problem in 3 sentences, the project scope is unclear -- return to Step 1.

2. **Vision is not a feature list.** Describe the outcome, not the implementation. "Users can generate narratives in 30 seconds instead of 15 minutes" beats "Build a RAG pipeline with Qdrant."

3. **Scope boundaries prevent rework.** The "Out of scope" section is as important as "In scope." Name the tempting adjacencies that this project will NOT address.

4. **Success criteria must be testable.** Every criterion should have a binary pass/fail evaluation. "Improved UX" fails this test. "Page load under 2 seconds on mobile" passes.

5. **Risks are not weaknesses.** Naming risks demonstrates maturity. Every project has them. The mitigation column shows you have a plan.

6. **Key Decisions are living documentation.** Add decisions as they happen during the project. This prevents re-litigating settled questions.

7. **Omit sections that do not apply.** Consulting projects skip Architecture & Tech Stack. Internal tooling may skip Risks. Do not pad with "N/A" rows -- just remove the section.

### Step 4: Create Project

Use the GraphQL helper:

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh create-project \
  "<name>" "<description>" "<content>" \
  "<YOUR_LINEAR_TEAM_UUID>" \
  "<YOUR_LINEAR_USER_UUID>" \
  <priority> "<startDate>" "<targetDate>"
```

Parse the JSON output to get the project ID.

### Step 5: Link Issues (Optional)

If user specified existing issues to include:

```bash
# For each issue identifier, get UUID from linearis
linearis issues read <IDENTIFIER>
# Parse UUID from JSON output, then link issue to project via GraphQL:
curl -s -X POST "https://api.linear.app/graphql" \
  -H "Content-Type: application/json" \
  -H "Authorization: $LINEAR_API_TOKEN" \
  -d "{\"query\": \"mutation { issueUpdate(id: \\\"<issueUUID>\\\", input: { projectId: \\\"<projectId>\\\" }) { success } }\"}"
```

### Step 6: Link Initiative (Optional)

If an initiative was identified:

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh link-initiative "<initiativeId>" "<projectId>"
```

### Step 7: Create Milestones (Optional)

If the content includes Key Milestones, create them in Linear:

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh create-milestone "<projectId>" "<name>" "<targetDate>" "<description>"
```

### Step 8: Register in linear-context.json

After successful project creation, add or update the project entry in `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json`:

```json
{
  "<Project Name>": {
    "id": "<project-id-from-step-4>",
    "team": "<TEAM>",
    "status": "<state from checklist>",
    "repoPath": "<local repo path or null for consulting>",
    "github": "<org/repo or null>",
    "note": "<1-sentence description>",
    "triggers": ["<keyword1>", "<keyword2>", "<keyword3>"]
  }
}
```

This ensures the new project is discoverable by all Linear workflows (CreateIssue, PlanProject, ResolveIssue) for automatic project inference.

### Step 9: Report

Output the final summary after project creation.

---

## Error Handling

| Error | Resolution |
|-------|------------|
| Description exceeds 255 chars | Truncate to 255 chars; move details to `content` field |
| Missing target date | Re-prompt via AskUserQuestion with format guidance (YYYY-MM-DD) |
| Project name already exists | Ask user to confirm: rename, or update existing project |
| GraphQL helper fails | Check `LINEAR_API_TOKEN` in `~/.env.secrets` |
| Initiative not found | Skip initiative linking; inform user |
| Issue identifier not found | Log warning, continue with remaining issues |
| `linearis` CLI not found | Prompt user to install: `bun install -g linearis` |
| Problem statement unclear | Return to Step 1; do not fabricate a project overview with vague goals |

---

## Output Format

```
PROJECT CREATED: [Name]

| Field | Value |
|-------|-------|
| ID | [project-id] |
| Name | [name] |
| Priority | P[N] |
| Start | [date] |
| Target | [date] |
| Lead | [Your name] |
| Team | <TEAM> |
| Issues Linked | [N] |
| Milestones | [N] |
| Initiative | [name or "None"] |

CONTENT PREVIEW:
  Problem: [1 sentence from Problem Statement]
  Vision: [1 sentence from Vision]
  Success: [count] criteria defined
  Risks: [count] identified

CONTEXT REGISTERED: linear-context.json updated with [N] triggers

View in Linear: https://linear.app/<workspace>/project/[slug]
```

---

*Referenced by: SKILL.md (workflow routing table)*
*References: LinearContext.md, linear-context.json*
