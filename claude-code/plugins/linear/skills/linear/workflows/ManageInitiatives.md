# ManageInitiatives Workflow

Create, list, and link Linear initiatives for strategic project grouping.

## Triggers

- "create initiative", "new initiative", "add initiative"
- "link project to initiative", "list initiatives"

## Prerequisites

- [ ] `LINEAR_API_TOKEN` set in environment or `~/.env.secrets`
- [ ] GraphQL helper available at `${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh`

---

## Operations

### Create Initiative

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh create-initiative \
  "<name>" "<description>" \
  "<YOUR_LINEAR_USER_UUID>" \
  "<targetDate>" "<content>"
```

**Fields:**

| Field | Required | Default | Action |
|-------|----------|---------|--------|
| name | YES | -- | From user input |
| description | YES | -- | Short description (1-2 sentences) |
| ownerId | YES | `<YOUR_LINEAR_USER_UUID>` | Initiative owner |
| targetDate | RECOMMENDED | -- | Ask if not provided (YYYY-MM-DD format) |
| content | RECOMMENDED | -- | Rich markdown description of goals, scope, success criteria |
| status | YES | Active | One of: Active, Completed, Backlog |

**Workflow:**

1. Parse user request for initiative name and description
2. Ask for target date if not provided (use AskUserQuestion)
3. Compose content field with markdown describing initiative scope
4. Create initiative via GraphQL helper
5. Report initiative ID and details

### List Initiatives

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh list-initiatives
```

**Output format:**

```
INITIATIVES
===========

| ID | Name | Status | Target Date | Projects |
|----|------|--------|-------------|----------|
| <id> | <name> | Active | 2026-06-30 | Project A, Project B |
| <id> | <name> | Backlog | -- | None linked |

Total: [N] initiatives ([N] active, [N] backlog, [N] completed)
```

### Link Project to Initiative

```bash
${CLAUDE_PLUGIN_ROOT}/skills/linear/tools/linear-graphql.sh link-initiative "<initiativeId>" "<projectId>"
```

**Workflow:**

1. If user provides initiative name (not ID), list initiatives first to resolve the ID
2. If user provides project name (not ID), resolve project ID from `linear-context.json` or search Linear
3. Link the project to the initiative
4. Confirm the link with both names

**Output format:**

```
INITIATIVE LINKED
=================
Initiative: <name> (<id>)
Project: <name> (<id>)
Status: Linked successfully
```

---

## Error Handling

| Error | Resolution |
|-------|------------|
| GraphQL helper fails | Check `LINEAR_API_TOKEN` in `~/.env.secrets` |
| Initiative not found | List initiatives and ask user to select |
| Project not found | Search Linear for project name, ask user to confirm |
| Duplicate initiative name | Warn user, ask if they want to proceed or use existing |

---

## Output Format

Every ManageInitiatives execution produces output in this structure:

```
MANAGE INITIATIVES: [Operation]

OPERATION: [Create / List / Link]
RESULT: [Success / Failed]
DETAILS: [Initiative name, ID, linked projects]
```

---

*Referenced by: SKILL.md (workflow routing table)*
*References: linear-graphql.sh*
