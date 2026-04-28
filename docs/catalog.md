# Resource Catalog

Keep this as the human-readable record of what lives in the package.

| Name | Type | Path | Status | Purpose |
| --- | --- | --- | --- | --- |
| `autopilot` | Extension | `extensions/autopilot/index.ts` | active | Autopilot workflow extension with approvals, preferences, continuation manifests, and v2 workflow support. |
| `hello` | Extension | `extensions/hello/index.ts` | example | Smoke-test extension that exposes `/agentic-utilities` and `agentic_utilities_ping`. |
| `critical-bug-hunt.prompt` | Prompt | `prompts/critical-bug-hunt.prompt.md` | active | Recent-commit audit prompt for high-severity correctness bugs and minimal fixes. |
| `harness-audit` | Skill | `skills/harness-audit/SKILL.md` | active | Global-first skill with `~/.pi/agent/skills/harness-audit` symlinked here; audits repo harness readiness and fix gaps. |
| `scaffold-notes` | Skill | `skills/scaffold-notes/SKILL.md` | active | Maintenance skill for adding resources to this repo consistently. |

## Status labels

- `active`: intended for regular use.
- `experimental`: usable, but API or behavior may change.
- `archived`: retained for record/history, not loaded by default if excluded from `package.json#pi`.
- `example`: scaffold/sample resource.
