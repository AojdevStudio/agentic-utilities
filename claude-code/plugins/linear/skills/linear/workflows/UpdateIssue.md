# UpdateIssue Workflow

Update an existing Linear issue with proper tracking.

## Triggers

- "update issue", "change status", "modify task"
- "edit issue", "update [IDENTIFIER]"
- Any request to modify an existing issue

## Workflow Steps

### Step 1: Identify the Issue

Find the issue by:
1. **Identifier** (e.g., `<TEAM>-165`)
2. **Title search** if no identifier provided
3. **Recent issues** if context suggests

```bash
linearis issues read <IDENTIFIER>
```

### Step 2: Determine Updates

Common update types:
- **Status change**: In Progress, In Review, Done
- **Priority change**: Urgent, High, Normal, Low
- **Description update**: Add context or requirements
- **Due date change**: Extend or shorten timeline
- **Label update**: Add or remove labels
- **Assignment change**: Reassign (rare)

### Step 3: Apply Updates

**For status, priority, title, description, labels** — use linearis CLI directly:

```bash
linearis issues update <IDENTIFIER> \
  -s "[new status if changing]" \
  -p [new priority if changing] \
  -t "[new title if changing]" \
  -d "[new description if changing]" \
  --labels "[updated labels if changing]"
```

**For due date or estimate changes** — use the GraphQL helper (requires UUID):

```bash
# First, read the issue to get its UUID
linearis issues read <IDENTIFIER>
# Parse the "id" field from JSON output

# Then set due date/estimate
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date "<UUID>" "YYYY-MM-DD"
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-estimate "<UUID>" <points>
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date-and-estimate "<UUID>" "YYYY-MM-DD" <points>
```

## Output Format

After successful update:

```
Issue Updated: [IDENTIFIER] - [Title]

Changes Made:
- [Field]: [Old Value] -> [New Value]
- [Field]: [Old Value] -> [New Value]

URL: [Linear URL]
```

## Common Update Scenarios

### Move to In Progress
```bash
linearis issues update <TEAM>-165 -s "In Progress"
```

### Change Priority
```bash
linearis issues update <TEAM>-165 -p 1
```

### Extend Due Date
```bash
# Read issue to get UUID
linearis issues read <TEAM>-165
# Use UUID from output
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date "<UUID>" "YYYY-MM-DD"
```

### Add Labels
```bash
linearis issues update <TEAM>-165 --labels "Feature,Client-Facing"
```

### Change Status and Priority Together
```bash
linearis issues update <TEAM>-165 -s "In Progress" -p 2
```

## Error Handling

| Error | Resolution |
|-------|------------|
| Issue not found | Search by title or list recent issues |
| Invalid state | List valid states for the team |
| Permission denied | Verify issue is in accessible team |
| GraphQL helper fails | Check LINEAR_API_TOKEN in ~/.env.secrets |
