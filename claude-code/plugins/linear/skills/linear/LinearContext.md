# Linear Context Reference

Workflow documentation for Linear integration via `linearis` CLI. **Data mappings are in `linear-context.json`** (copy from `linear-context.example.json` and populate with your workspace data).

## Issue Creation Checklist (MANDATORY)

**When creating any Linear issue, ALL fields must be populated:**

| Field | Required | Action |
|-------|----------|--------|
| `title` | YES | Clear, actionable title |
| `team` | YES | Infer from context triggers in JSON |
| `project` | YES | Match using project triggers in JSON |
| `assignee` | YES | Always your UUID from `linear-context.json` user.id |
| `priority` | YES | 1=Urgent, 2=High, 3=Normal, 4=Low |
| `dueDate` | YES | ALWAYS prompt user if not provided |
| `estimate` | RECOMMENDED | Story points: 1, 2, 3, 5, 8 |
| `labels` | RECOMMENDED | Match using label triggers in JSON |

## Project Creation

See `workflows/CreateProject.md` for the full project creation workflow, mandatory field checklist, and content template.

**Key reminders:**
- `description` = max 255 chars (list views). `content` = unlimited markdown (full project overview).
- Do NOT create separate "Project Brief" documents -- use the `content` field.
- Content template starts with Problem Statement + Vision (not just a feature list).
- Every project content must include: Problem Statement, Vision, Scope, Success Criteria.
- Architecture/Tech Stack, Risks, Dependencies, Key Decisions are included when applicable.
- After creation, register the project in `linear-context.json` with triggers for auto-inference.
- Priority: 0=None, 1=Urgent, 2=High, 3=Normal, 4=Low.

## Workflow

```
1. Parse user request for keywords
2. Match keywords -> project triggers in linear-context.json
3. If no match -> ASK user which project (use AskUserQuestion)
4. Prompt for due date if not provided
5. Set priority (default: 3=Normal)
6. Create issue with linearis CLI, then set due date/estimate via GraphQL helper
```

## Inference Examples

| User says... | Matches trigger | Project |
|--------------|-----------------|---------|
| "Fix KPI dashboard metrics" | "kpi", "dashboard" | My Dashboard Project |
| "Update consulting deliverable" | "consulting", "deliverable" | My Consulting Project |
| "Add church livestream feature" | "church", "livestream" | My Media Project |

*(Populate your own trigger table in `linear-context.json` — this table shows the inference pattern.)*

## Issue Format

See `IssueTemplate.md` for the full structured template (Summary, Current/Expected Behavior, AC max 3, Scope).

```markdown
## Summary
1-2 sentences: what and why.

## Current Behavior
What happens now.

## Expected Behavior
What should happen after resolution.

## Acceptance Criteria (max 3)
- [ ] AC-1: [Testable outcome]
- [ ] AC-2: [Testable outcome]
- [ ] AC-3: [Testable outcome]

## Scope
**In scope:** [inclusions]
**Out of scope:** [exclusions]
```

## CLI Reference

### Create Issue (Two-Step)

```bash
# Step 1: Create issue via linearis
linearis issues create "Issue title" \
  -d "Description" \
  -a <YOUR_LINEAR_USER_UUID> \
  -p 3 \
  --team <TEAM_KEY> \
  --project "<Project Name>" \
  --labels "Feature"

# Step 2: Parse UUID from JSON output, then set due date & estimate
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date-and-estimate "<UUID>" "YYYY-MM-DD" 3
```

### Search Issues

```bash
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  --team <TEAM_KEY> \
  --status "In Progress,Todo" \
  -l 25
```

### Read Issue

```bash
linearis issues read <TEAM>-165
```

### Update Issue

```bash
# Status, priority, labels via CLI
linearis issues update <TEAM>-165 -s "In Progress" -p 2 --labels "Feature"

# Due date/estimate via GraphQL helper (requires UUID)
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date "<UUID>" "YYYY-MM-DD"
```

### Close Issue

```bash
linearis issues update <TEAM>-165 -s "Done"
```

## Important Notes

- `linearis issues list` has **NO filters** — always use `search` when filtering
- Due date and estimate require the GraphQL helper (linearis has no flags for these)
- The GraphQL helper requires the issue **UUID**, not the identifier — read the issue first
- Assignee must be the UUID from `linear-context.json`, not `"me"`

## Development Lifecycle

### Branch Naming Convention

Feature branches auto-link to Linear issues via naming:

```
feature/<ISSUE-ID>-kebab-title
```

**Examples:**
- `feature/<team>-123-add-login-form`
- `feature/<team>-456-fix-search-duplicates`

### Status Transitions

```
Backlog → Todo → In Progress → In Review → Done
```

| Transition | Triggered By |
|-----------|-------------|
| → In Progress | ResolveIssue Phase 2 (branch created) |
| → In Review | ResolveIssue Phase 8 (PR created) |
| → Done | Manual close or post-merge |

### TDD Workflow (Enforced by ResolveIssue)

```
Red    → Write failing tests matching acceptance criteria
Green  → Implement code to make tests pass
Refactor → Clean up while keeping tests green
```

Every ResolveIssue run follows TDD. Tests are written first, implementation second.

### Project-to-Repo Mapping

Each project in `linear-context.json` has a `repoPath` field pointing to the local git repo. ResolveIssue uses this to know where to work. Projects with `repoPath: null` are consulting/non-code projects — ResolveIssue skips them.

---

*Data: `linear-context.json` | See `linear-context.example.json` for schema reference*
