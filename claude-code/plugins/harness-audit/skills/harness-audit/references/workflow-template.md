# Minimal `WORKFLOW.md` Template

Use this as a starting point when `audit+fix:symphony` needs to seed a repo-local Symphony contract. Keep placeholders obvious. Do not add real secrets.

```md
---
tracker:
  kind: "REPLACE_WITH_TRACKER_KIND" # examples: linear, github, jira, custom
  project_slug: "REPLACE_WITH_PROJECT_SLUG"
  api_key: $LINEAR_API_KEY
  active_states:
    - Todo
    - In Progress
    - Rework
    - Merging
  terminal_states:
    - Done
    - Closed
    - Cancelled
    - Canceled
    - Duplicate
polling:
  interval_ms: 30000
workspace:
  root: "REPLACE_WITH_WORKSPACE_ROOT"
hooks:
  after_create: |
    git clone REPLACE_WITH_REPO_URL .
    ./scripts/bootstrap.sh
  before_run: |
    ./scripts/verify-ready.sh
agent:
  max_concurrent_agents: 2
  max_turns: 20
  max_retry_backoff_ms: 300000
agent_runner:
  kind: "REPLACE_WITH_AGENT_RUNNER_KIND" # examples: codex, claude, pi, custom
  command: "REPLACE_WITH_AGENT_APP_SERVER_COMMAND"
  sandbox: "REPLACE_WITH_SANDBOX_OR_APPROVAL_POLICY"
---

You are working on ticket `{{ issue.identifier }}`.

Title: {{ issue.title }}
State: {{ issue.state }}
URL: {{ issue.url }}
Labels: {{ issue.labels }}

Description:
{% if issue.description %}
{{ issue.description }}
{% else %}
No description provided.
{% endif %}

## Operating contract

- Work only inside the provided workspace.
- Start by reading repo-local agent instructions (`AGENTS.md`, repo skills, and rules docs).
- Maintain one persistent workpad/comment for plan, acceptance criteria, validation, evidence, and blockers.
- Before edits, capture a reproduction signal or state why reproduction is not applicable.
- Keep scope tied to the ticket acceptance criteria.
- File follow-up work separately instead of expanding scope.
- Run the repo's required validation before handoff.
- Produce compressed evidence a human can review quickly.
- Stop only for true blockers: missing auth, missing required secrets, or impossible acceptance criteria.

## State flow

- `Todo`: move to `In Progress`, create/update workpad, then execute.
- `In Progress`: continue from current workpad and workspace state.
- `Rework`: re-read feedback, start from a clean branch/workspace when policy requires it, then execute.
- `Human Review`: do not code unless feedback moves the ticket back to `Rework`.
- `Merging`: follow the repo landing skill; do not bypass required merge policy.
- Terminal states: do nothing.

## Required preflight

Before unattended execution, run the tracker preflight and fix every error:

```bash
bun run symphony validate WORKFLOW.md --live-tracker
```

This must verify the tracker API key, project slug, active states, terminal states, and lifecycle states against the real tracker workflow. Do not start polling if preflight fails.

## Required handoff

Before moving to human review, ensure:

- plan/checklist is current and complete
- acceptance criteria are checked off
- validation command(s) and results are recorded
- UI/runtime evidence is attached when behavior is user-facing
- PR is opened or updated, if this repo uses PRs
- review comments and CI failures are resolved or explicitly pushed back with rationale
```

## Template rules

- Keep this file repo-local and versioned.
- Prefer placeholders over environment-specific values.
- The `after_create` hook must be safe in an empty workspace.
- Do not hardcode personal API tokens, local absolute repo paths, or private machine names.
- Customize `tracker.kind` and state names to the actual tracker workflow.
- Include a live tracker preflight command for the actual orchestrator; for Symphony + Linear use `bun run symphony validate WORKFLOW.md --live-tracker`.
- If targeting OpenAI's Elixir reference implementation, use `tracker.kind: linear` and adapt `agent_runner` to its current `codex` front-matter schema. This generic template is intentionally runner-adapter-neutral.
