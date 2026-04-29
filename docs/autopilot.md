# Autopilot Commands

Autopilot is the Pi extension behind the `/autopilot` slash command. It supports v2 planning/workflow commands plus older legacy runner commands kept for compatibility.

After changing the extension, run `/reload` in Pi so autocomplete and docs-backed behavior refresh.

## Primary v2 commands

| What it is | Command | What it does |
| --- | --- | --- |
| Status | `/autopilot status` | Shows repo enablement, config, latest workflow, and slice completion status. |
| Workflow list | `/autopilot workflows` | Lists known v2 workflows. Alias: `/autopilot workflow`. |
| Planning workflow | `/autopilot plan <idea-or-source>` | Creates a v2 planning workflow from an idea, URL, issue, repo, package, or plan. |
| Architecture workflow | `/autopilot architecture <scope>` | Creates a v2 architecture/refactor workflow for a codebase area or friction point. Alias: `/autopilot arch`. |
| Issue gate approval | `/autopilot approve issues <workflow-id> [note]` | Approves the issue-drafting gate for a workflow. |
| Execution gate approval | `/autopilot approve execution <workflow-id> [note]` | Approves the execution gate for a workflow. |
| Phase transition | `/autopilot transition <workflow-id> <phase> [--force] [note]` | Moves a workflow to another phase after validating the transition. |
| Concept lock | `/autopilot concept-lock <workflow-id> [summary]` | Locks the planning concept and sends the artifact-drafting prompt. Alias: `/autopilot lock-concept`. |
| Ship next slice | `/autopilot ship <workflow-id>` | Claims and runs the next approved execution issue in a fresh worktree. |
| Resume workflow prompt | `/autopilot resume-workflow <workflow-id>` | Re-sends the lane prompt for an existing v2 workflow. |
| Repo setup | `/autopilot setup` | Configures Autopilot for the repo interactively. Alias: `/autopilot prefs`. |
| Repo setup with flags | `/autopilot setup --source <plan|github|linear|mixed> --verify <conservative|normal|strict> --allow <globs> --deny <globs>` | Configures source priority, verification profile, allowed paths, and denied paths without the full prompt flow. |
| Continue from source | `/autopilot continue <plan-path-or-issue-ref>` | Builds a continuation manifest from a plan, GitHub issue, Linear issue, or supported source and starts/continues work. |
| Continue from GitHub | `/autopilot from-gh <issue-ref-or-url>` | GitHub-focused continuation path. Accepts refs like `#123`, `owner/repo#123`, or issue URLs. |
| Continue from Linear | `/autopilot from-linear <issue-ref-or-url>` | Linear-focused continuation path. Accepts keys like `ENG-123` or Linear URLs. |

## Legacy runner commands

These commands operate on the older manifest/runner flow. Prefer the v2 commands above for new work.

| What it is | Command | What it does |
| --- | --- | --- |
| Legacy scaffold | `/autopilot scaffold <slug-or-manifest-path>` | Creates an old-style Autopilot manifest scaffold. Alias: `/autopilot init`. |
| Legacy planning | `/autopilot legacy-plan <idea>` | Starts the older planning flow and writes a plan under `.pi/autopilot/plans/`. Alias: `/autopilot plan-legacy`. |
| Legacy manifest from plan | `/autopilot from-plan <plan-path>` | Converts a plan file into an old-style manifest. Alias: `/autopilot draft`. |
| Legacy start | `/autopilot start <manifest-path-or-source>` | Starts the old runner from a manifest, plan, GitHub issue, or supported source. |
| Legacy pause | `/autopilot pause` | Pauses the active old runner. |
| Legacy resume | `/autopilot resume` | Resumes the active old runner from in-memory state. |
| Legacy stop | `/autopilot stop` | Stops the active old runner. |
| Legacy checkpoint | `/autopilot checkpoint` | Writes a checkpoint for the active old runner. |
| Legacy takeover | `/autopilot takeover <manifest-path>` | Force-takes over an old runner manifest. |

## Common v2 flow

```text
/autopilot setup
/autopilot plan <idea-or-source>
/autopilot status
/autopilot concept-lock <workflow-id> <summary>
/autopilot approve issues <workflow-id>
/autopilot approve execution <workflow-id>
/autopilot ship <workflow-id>
```

## Transition phases

Planning workflows use phases like `intake`, `discovery`, `grill`, `prd-draft`, `glossary-draft`, `agreement`, `issue-approval`, `issues-created`, `triage`, `execution-approval`, `ready-to-execute`, `done`, and `blocked`.

Architecture workflows use phases like `intake`, `explore`, `candidates`, `interface-design`, `refactor-rfc`, `issue-approval`, `triage`, `execution-approval`, `ready-to-execute`, `done`, and `blocked`.
