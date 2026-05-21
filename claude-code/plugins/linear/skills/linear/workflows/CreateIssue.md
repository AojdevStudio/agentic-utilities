# CreateIssue Workflow

Create a new Linear issue with proper assignment, due dates, and context.

## Triggers

- "create issue", "new task", "add to linear"
- "track this", "create a ticket", "log this as an issue"
- Any request to create work items in Linear

## Prerequisites

Before creating an issue, ensure you have:
- [ ] Issue title (from user or inferred)
- [ ] Team (inferred or asked)
- [ ] Due date (ALWAYS ask)
- [ ] Description/context

## Workflow Steps

### Step 1: Set Team

Read the `defaultTeamKey` from `linear-context.json` routing section. If multiple active teams exist, infer from context or ask.

### Step 2: Prompt for Due Date

**MANDATORY**: Always ask for a due date. Never create an issue without one.

```
What should be the due date for this issue?
```

Suggest options based on complexity:
- Simple task: 2-3 days
- Medium task: 1 week
- Complex task: 2 weeks

### Step 3: Infer Project

Match conversation keywords to project triggers in `linear-context.json`:

Read the `projects` section and match against each project's `triggers` array. Present a few example matches from the user's own `linear-context.json`.

**If no match found**: Ask the user which project (use AskUserQuestion).

### Step 4: Determine Labels

Apply labels based on content:

| Context | Label(s) |
|---------|----------|
| Bug reports, errors, broken, crash | Bug |
| New capabilities, new feature | Feature |
| Optimization, refactoring | Improvement |
| Client deliverables | Client-Facing + Deliverable |
| HR, payroll, benefits, salary | HR/Compensation |
| SOP, workflow, process | Operations |
| Training, onboarding | Training |
| Milestone, checkpoint, review | Milestone |

### Step 5: Format Description

Use the **IssueTemplate** format from `${CLAUDE_PLUGIN_ROOT}/skills/linear/IssueTemplate.md`. Every issue description must include the Context section:

```markdown
## Summary
[1-2 sentences: what gets accomplished and why]

## Context
- **Repository:** <local path from linear-context.json>
- **GitHub:** <org/repo from linear-context.json>
- **Relevant Files:** <comma-separated file paths, or "To be determined during investigation">
- **Related Issues:** <identifiers or "None">

## Current Behavior
[What happens now. Use "N/A" for new features.]

## Expected Behavior
[What should happen after this issue is resolved.]

## Acceptance Criteria (max 3)
- [ ] AC-1: [Testable outcome -- one sentence, binary pass/fail]
- [ ] AC-2: [Testable outcome]
- [ ] AC-3: [Testable outcome]

## Scope
**In scope:** [explicit inclusions]
**Out of scope:** [explicit exclusions]

## Additional Context
[Logs, screenshots, error messages, design links.]
```

### Step 5b: Auto-Populate Context Section

After inferring the project in Step 3, read `${CLAUDE_PLUGIN_ROOT}/skills/linear/linear-context.json` to populate the Context section:

1. Match the inferred project to its entry in `linear-context.json`
2. Set **Repository** to the project's `repoPath` value
3. Set **GitHub** to the project's `github` value (format: `org/repo`)
4. If the user's description mentions specific files, include them in **Relevant Files**; otherwise use `"To be determined during investigation"`
5. If the user mentions related issues (e.g., `<TEAM>-123`), include them in **Related Issues**; otherwise use `"None"`

**For consulting projects** (where `repoPath` is `null`):
- Set `Repository: N/A (consulting project)`
- Set `GitHub: N/A`
- Relevant Files and Related Issues follow the same rules

### Step 6: Deduplication Check

**MANDATORY**: Before creating the issue, search for existing issues with similar titles to prevent duplicates.

#### 6.1 Extract title keywords

Take the issue title and extract 2-3 significant keywords (drop articles, prepositions, and common words like "add", "fix", "update").

#### 6.2 Search for existing issues

```bash
linearis issues list | grep -i "<keyword>"
```

#### 6.3 Evaluate matches

| Result | Action |
|--------|--------|
| No matches found | Proceed to Step 7 (Create Issue). |
| 1+ matches found | Present matches to the user (identifier, title, status). Use AskUserQuestion: "Found N existing issue(s) with similar titles. Create anyway or skip?" |
| User says "create anyway" | Proceed to Step 7. |
| User says "skip" | Stop. Do not create the issue. Report the matching issue(s). |

### Step 7: Create Issue (Two-Step)

**Step 7a: Create via linearis CLI**

```bash
linearis issues create "[Inferred or provided title]" \
  -d "[Formatted description]" \
  -a <YOUR_LINEAR_USER_UUID> \
  -p [1-4] \
  --team [Team key] \
  --project "[If applicable]" \
  --labels "[Inferred labels]"
```

Parse the JSON output to extract the issue UUID (`id` field).

**Step 7b: Set due date and estimate via GraphQL helper**

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date-and-estimate "[issue UUID]" "[User-provided date]" [estimate points]
```

If no estimate was provided, use `set-due-date` only:

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh set-due-date "[issue UUID]" "[User-provided date]"
```

## Output Format

After successful creation:

```
Issue Created: [IDENTIFIER] - [Title]

Team: [Team Name]
Assignee: [Your name]
Due Date: [Date]
Priority: [Priority Level]
Labels: [Labels]
URL: [Linear URL]
```

## Validation Checklist

Before creating the issue, verify:

- [ ] Title is clear and actionable (starts with a verb)
- [ ] Due date is set (MANDATORY -- always ask)
- [ ] Description follows IssueTemplate format
- [ ] **For code projects** (`repoPath` is not null): Context section has Repository and GitHub fields populated from `linear-context.json`
- [ ] **For consulting projects** (`repoPath` is null): Context section uses `Repository: N/A (consulting project)`
- [ ] Acceptance criteria are binary testable (max 3)
- [ ] Scope section has explicit in/out boundaries

## Error Handling

| Error | Resolution |
|-------|------------|
| Team not found | Ask user to specify team |
| Invalid due date | Re-prompt with format guidance (YYYY-MM-DD) |
| Missing required field | Prompt for missing information |
| GraphQL helper fails | Check LINEAR_API_TOKEN in ~/.env.secrets |
| Context section missing Repository/GitHub | Read `linear-context.json` and populate before creating |
