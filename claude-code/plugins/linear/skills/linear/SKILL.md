---
name: linear
description: Linear issue management via linearis CLI with team inference. USE WHEN linear, issues, tasks, tickets, backlog, sprint, resolve issue, ship issue, plan project, break down feature, ship daily, what should I work on.
---

# Linear

**BLOCKING skill** that intercepts ALL Linear operations via `linearis` CLI to enforce:
- Issues assigned to the configured user
- Due dates always requested
- Team/project inferred from conversation context

## CLI Tooling

### linearis CLI

All Linear operations use the `linearis` CLI which outputs JSON:

```bash
linearis issues create "<title>" -d "<desc>" -a <YOUR_LINEAR_USER_UUID> -p <N> --team <KEY> --project "<name>" --labels "<labels>"
linearis issues search "<query>" --assignee <YOUR_LINEAR_USER_UUID> --team <KEY> --status "<states>" -l 25
linearis issues list -l 25
linearis issues read <IDENTIFIER>
linearis issues update <IDENTIFIER> -s "<status>" -p <N> --labels "<labels>"
```

### User UUID

Set your Linear user UUID in `linear-context.json` under `user.id`. Use this UUID instead of `"me"` for all assignee operations for reliable results.

### GraphQL Helper (Due Date & Estimate)

`linearis` does not support `--due-date` or `--estimate` flags. Use the GraphQL helper bundled in this plugin:

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date "<issueUUID>" "YYYY-MM-DD"
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-estimate "<issueUUID>" <points>
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date-and-estimate "<issueUUID>" "YYYY-MM-DD" <points>
```

Requires `LINEAR_API_TOKEN` in environment or `~/.env.secrets`.

### Important CLI Notes

- `linearis issues list` has **NO filters** — use `linearis issues search` for all filtered queries
- `linearis issues search` supports `--team`, `--assignee`, `--project`, `--status`, `-l`
- `linearis issues update` accepts both UUID and identifiers (e.g., `<TEAM>-165`)
- Due date/estimate changes require the issue **UUID** (not identifier) — read the issue first to get it

## Workflow Routing

| Workflow | Trigger | File |
|----------|---------|------|
| **CreateIssue** | "create issue", "new task", "add to linear", "track this" | `workflows/CreateIssue.md` |
| **CreateProject** | "create project", "new project", "add project", "start a project" | `workflows/CreateProject.md` |
| **UpdateIssue** | "update issue", "change status", "modify task", "edit issue" | `workflows/UpdateIssue.md` |
| **ListIssues** | "list issues", "show tasks", "what's in backlog", "my issues" | `workflows/ListIssues.md` |
| **CloseIssue** | "close issue", "mark done", "complete task", "finish issue" | `workflows/CloseIssue.md` |
| **ResolveIssue** | "resolve issue", "work on <TEAM>-123", "fix issue", "ship issue" | `/resolve <TEAM>-123` command (preferred) or `workflows/ResolveIssue.md` |
| **PlanProject** | "plan project", "break this down", "decompose feature", "create project plan" | `workflows/PlanProject.md` |
| **ManageInitiatives** | "create initiative", "list initiatives", "link project to initiative" | `workflows/ManageInitiatives.md` |
| **ManageMilestones** | "create milestone", "add milestone", "project milestone" | `workflows/ManageMilestones.md` |
| **ShipDaily** | "ship daily", "what should I work on", "daily standup", "ship" | `workflows/ShipDaily.md` |
| **MonitorCI** | "check CI", "CI status", "fix CI", "is CI passing" | `workflows/MonitorCI.md` |
| **ReviewPR** | "handle review", "PR feedback", "address review comments" | `workflows/ReviewPR.md` |

## Examples

**Example 1: Create a new issue**
```
User: "Create an issue to update the API documentation"
-> Invokes CreateIssue workflow
-> Infers team from context or prompts
-> Prompts for due date
-> Assigns to configured user UUID
-> Creates issue via linearis CLI + GraphQL helper for due date
```

**Example 2: List current tasks**
```
User: "What issues do I have in my backlog?"
-> Invokes ListIssues workflow
-> Searches with --assignee UUID
-> Fetches issues across all teams
-> Formats with priority indicators
```

**Example 3: Close a completed task**
```
User: "Mark <TEAM>-165 as done"
-> Invokes CloseIssue workflow
-> Identifies issue by identifier
-> Updates status to "Done" via linearis issues update
```

**Example 4: Resolve an issue end-to-end**
```
User: "Resolve <TEAM>-200"
-> Fetches issue, looks up repo from linear-context.json
-> Creates feature branch, updates status to In Progress
-> Architect agent investigates codebase, proposes TDD strategy
-> Engineer agent writes failing tests, then implements solution
-> Commits, pushes, creates PR with issue reference
-> Monitors CI, fixes any failures via subagent
-> CI passes: immediately attempts merge (squash, delete branch)
-> If merge blocked: attempts self-approve, then merges
-> Only pauses if external reviewer required (re-enter with /resolve <TEAM>-200)
-> On approval: merges PR, closes Linear issue to Done
```

**Example 5: Plan a new feature as issues**
```
User: "Break down the search feature into issues"
-> Invokes PlanProject workflow
-> Decomposes feature using First Principles thinking
-> Creates 5 issues with max 3 acceptance criteria each
-> Sets dependencies, priorities, and estimates
-> Batch-creates in Linear after user approval
```

**Example 6: Daily shipping workflow**
```
User: "What should I work on today?"
-> Invokes ShipDaily workflow
-> Fetches open issues sorted by priority + due date
-> Presents top 3 candidates for selection
-> Triggers ResolveIssue for each selected issue
-> Reports daily shipping summary
```

**Example 7: Check CI on a PR**
```
User: "Is CI passing on <TEAM>-200?"
-> Invokes MonitorCI workflow
-> Checks GitHub Actions status for the feature branch
-> Reports: passing/failing with details
```

**Example 8: Handle PR review feedback**
```
User: "Address the review comments on <TEAM>-200"
-> Invokes ReviewPR workflow
-> Reads all review comments from GitHub
-> Spawns subagent to address each comment
-> Commits fixes, pushes, reports status
```

## Validation Rules (ENFORCED)

### On Issue Creation - MANDATORY CHECKLIST
**ALL fields must be populated. No partial issues.**

| Field | Required | Action |
|-------|----------|--------|
| `title` | YES | Clear, actionable title |
| `team` | YES | Infer from context triggers or ASK |
| `project` | YES | Match using linear-context.json triggers |
| `assignee` | YES | Always use your UUID from linear-context.json |
| `priority` | YES | 1=Urgent, 2=High, 3=Normal, 4=Low |
| `dueDate` | YES | ALWAYS prompt user if not provided |
| `estimate` | RECOMMENDED | Story points (1, 2, 3, 5, 8) |
| `labels` | RECOMMENDED | Based on label logic |

**Workflow:**
1. Parse user request for project context triggers
2. Match to project using linear-context.json
3. If no match -> ASK user which project (use AskUserQuestion)
4. Prompt for due date if not provided
5. Set priority (default: 3=Normal)
6. Create issue with linearis CLI, then set due date/estimate via GraphQL helper

### On Issue Update
1. Track what changed

### On Issue Close
1. Verify completion

## Team Routing

All issues route to the default team configured in `linear-context.json`. The `defaultTeamKey` field in the `routing` section determines which team key is used.

## Project Inference

The skill infers project from conversation context using triggers in `linear-context.json`. Each project entry has a `triggers` array of keywords. If no project match is found, **ASK the user** which project (use AskUserQuestion).

## Label Inference

| Context | Labels |
|---------|--------|
| Bug reports, errors, broken | Bug |
| New capabilities | Feature |
| Optimization, refactoring | Improvement |
| Client deliverables | Client-Facing + Deliverable |
| Training materials | Training |
| HR, payroll, benefits | HR/Compensation |
| Business workflows | Operations |
| Key checkpoints | Milestone |

## Gotchas

- linearis CLI outputs JSON by default — do NOT add `--json` flag. It doesn't exist and errors out.
- Project descriptions have a 255 character limit. Truncate before sending.
- Multiline issue descriptions can cause JSON parse errors when extracting issue IDs. Use `head -3` for id extraction from linearis output.
- Due dates must be set via GraphQL: `${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date`

## Reference Documentation

- **`linear-context.json`** - Your workspace data: teams, projects, triggers, IDs (copy from `linear-context.example.json` and populate)
- **`LinearContext.md`** - Workflow documentation and examples
