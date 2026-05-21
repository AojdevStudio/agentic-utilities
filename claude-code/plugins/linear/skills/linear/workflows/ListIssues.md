# ListIssues Workflow

List and search Linear issues with smart defaults.

## Triggers

- "list issues", "show my tasks", "what's in backlog"
- "my issues", "what am I working on"
- "show [team] issues", "list [project] tasks"

## Workflow Steps

### Step 1: Apply Smart Defaults

**Default filters** (always applied unless overridden):
- `--assignee <YOUR_LINEAR_USER_UUID>` - Show only user's issues
- Exclude archived issues (default behavior)

### Step 2: Infer Additional Filters

Based on context:

| User Says | Filter Applied |
|-----------|---------------|
| "backlog" | `--status "Backlog"` |
| "in progress" | `--status "In Progress"` |
| "todo" | `--status "Todo"` |
| "done", "completed" | `--status "Done"` |
| "urgent" | Post-filter by `priority: 1` in JSON output |
| "high priority" | Post-filter by `priority: 2` in JSON output |
| "due today" | Post-filter by `dueDate` matching today |
| "overdue" | Post-filter by `dueDate` < today |
| "[team name]" | `--team "[team key]"` |
| "[project name]" | `--project "[project]"` |

### Step 3: Execute Query

**For filtered queries** (most cases), use `search`:

```bash
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  --team "[if specified]" \
  --project "[if specified]" \
  --status "[if specified]" \
  -l 25
```

**For unfiltered listing** (rare — "show everything"):

```bash
linearis issues list -l 25
```

**Important:** `linearis issues list` has **NO filter flags**. Always use `search` when any filter is needed.

### Step 4: Post-Query Filtering

For filters not supported by `search` (priority, due date), parse the JSON output:

- **Priority filter**: Check `priority` field in each issue (1=Urgent, 2=High, 3=Normal, 4=Low)
- **Overdue filter**: Compare `dueDate` field against today's date
- **Due today**: Match `dueDate` to current date

### Step 5: Format Output

Group by status and priority:

```
MY LINEAR ISSUES

IN PROGRESS (3)
  [P1] <TEAM>-165: Fix search issue (Due: Dec 27)
  [P2] <TEAM>-164: Update documentation (Due: Dec 30)
  [P3] <TEAM>-163: Add export feature (Due: Jan 5)

TODO (5)
  [P2] <TEAM>-162: Create onboarding flow (Due: Dec 28)
  ...

BACKLOG (12)
  ...

Total: 20 issues
```

## Priority Indicators

| Priority | Display |
|----------|---------|
| 1 (Urgent) | `[P1]` + highlight |
| 2 (High) | `[P2]` |
| 3 (Normal) | `[P3]` |
| 4 (Low) | `[P4]` |
| 0 (None) | `[-]` |

## Due Date Indicators

| Status | Display |
|--------|---------|
| Overdue | `(OVERDUE: Dec 15)` |
| Due today | `(Due: TODAY)` |
| Due this week | `(Due: Dec 28)` |
| No due date | `(No due date)` |

## Quick Filters

### Show Urgent Items
```bash
# Search all assigned issues, then post-filter for priority 1
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  -l 50
# Then filter JSON output where priority == 1
```

### Show Overdue Items
```bash
# Search open issues, then post-filter by dueDate < today
linearis issues search "" \
  --assignee <YOUR_LINEAR_USER_UUID> \
  --status "Todo,In Progress,Backlog" \
  -l 50
# Then filter JSON output where dueDate < current date
```

## Output Variations

### Compact View
```
<TEAM>-165 [P1] Fix search issue -> In Progress
<TEAM>-164 [P2] Update documentation -> Todo
```

### Detailed View
```
<TEAM>-165: Fix search issue
  Team: [Team Name] | Project: [Project Name]
  Status: In Progress | Priority: High
  Due: Dec 27, 2025 | Labels: Bug
  URL: https://linear.app/<workspace>/issue/<TEAM>-165
```
