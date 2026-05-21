# linear

Linear issue management via the `linearis` CLI — create, update, close, resolve, and plan issues with team inference, TDD-driven resolution, CI monitoring, and PR review handling. A **blocking** skill that intercepts all Linear operations so issues are always assigned, always have a due date, and always land in the right team and project.

## What it does

The skill routes a request to one of 12 workflow playbooks:

| Workflow | Trigger |
|----------|---------|
| CreateIssue | "create issue", "new task", "track this" |
| CreateProject | "create project", "new project" |
| UpdateIssue | "update issue", "change status" |
| ListIssues | "list issues", "what's in backlog" |
| CloseIssue | "close issue", "mark done" |
| ResolveIssue | "resolve issue", "work on TEAM-123", "ship issue" |
| PlanProject | "plan project", "break this down" |
| ManageInitiatives | "create initiative", "link project to initiative" |
| ManageMilestones | "create milestone", "project milestone" |
| ShipDaily | "what should I work on", "daily standup" |
| MonitorCI | "check CI", "is CI passing" |
| ReviewPR | "handle review", "address review comments" |

`ResolveIssue` runs an end-to-end TDD loop: branch → failing tests → implementation → PR → CI monitoring → merge.

## Setup

1. Install the `linearis` CLI and make it available on `PATH`.
2. Copy `skills/linear/linear-context.example.json` to `linear-context.json` and populate it with your teams, projects, triggers, and Linear user UUID.
3. Set `LINEAR_API_TOKEN` in your environment (or `~/.env.secrets`) — required by the GraphQL helper for due-date and estimate operations.

## Bundled content

```
skills/linear/
├── SKILL.md                          # blocking skill — CLI rules + workflow routing
├── LinearContext.md                  # workspace-context documentation
├── IssueTemplate.md                  # issue body template
├── linear-context.example.json       # template config (copy + populate locally)
├── tools/
│   └── linear-graphql.sh             # GraphQL helper for due dates + estimates
└── workflows/                        # 12 workflow playbooks (see table above)
```

## License

MIT — see repository LICENSE.
