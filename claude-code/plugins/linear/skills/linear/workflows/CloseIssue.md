# CloseIssue Workflow

Close/complete a Linear issue with proper status update.

## Triggers

- "close issue", "mark done", "complete task"
- "finish [IDENTIFIER]", "[IDENTIFIER] is done"
- "mark [IDENTIFIER] as complete"

## Workflow Steps

### Step 1: Identify the Issue

Find the issue by:
1. **Identifier** mentioned (e.g., `<TEAM>-165`)
2. **Title search** if no identifier
3. **Most recently discussed** issue in conversation

```bash
linearis issues read <IDENTIFIER>
```

### Step 2: Verify Completion

Before closing, optionally confirm:
- All acceptance criteria met
- No blocking issues remain
- Ready for closure

### Step 3: Update Status to Done

```bash
linearis issues update <IDENTIFIER> -s "Done"
```

## Output Format

After successful closure:

```
Issue Closed: [IDENTIFIER] - [Title]

Status: Done
Completed: [Current Date]
URL: [Linear URL]
```

## Alternative Close States

Sometimes issues aren't "Done" but need closing:

### Cancel Issue
```bash
linearis issues update <IDENTIFIER> -s "Canceled"
```

### Mark as Duplicate
```bash
linearis issues update <IDENTIFIER> -s "Duplicate"
```

## Batch Close

For closing multiple issues:

```
User: "Close <TEAM>-165, <TEAM>-166, and <TEAM>-167"

-> Process each issue:
   1. linearis issues update <TEAM>-165 -s "Done"
   2. linearis issues update <TEAM>-166 -s "Done"
   3. linearis issues update <TEAM>-167 -s "Done"
```

## Error Handling

| Error | Resolution |
|-------|------------|
| Issue not found | Search by title or list recent issues |
| Already closed | Inform user issue is already done |
| Invalid state transition | Show valid state options |
