# Resource Catalog

Keep this as the human-readable record of what lives in the package.

| Name | Type | Path | Status | Purpose |
| --- | --- | --- | --- | --- |
| `agentic-utilities` | Claude Code Marketplace | `.claude-plugin/marketplace.json` | active | Marketplace manifest exposing Claude Code plugins from this repo. |
| `autopilot` | Extension | `extensions/autopilot/index.ts` | active | Autopilot workflow extension with approvals, preferences, continuation manifests, v2 workflow support, and command docs in `docs/autopilot.md`. |
| `hello` | Extension | `extensions/hello/index.ts` | example | Smoke-test extension that exposes `/agentic-utilities` and `agentic_utilities_ping`. |
| `critical-bug-hunt.prompt` | Prompt | `prompts/critical-bug-hunt.prompt.md` | active | Recent-commit audit prompt for high-severity correctness bugs and minimal fixes. |
| `harness-audit` | Skill | `skills/harness-audit/SKILL.md` | active | Global-first skill with `~/.pi/agent/skills/harness-audit` symlinked here; audits repo harness readiness and fix gaps. |
| `scaffold-notes` | Skill | `skills/scaffold-notes/SKILL.md` | active | Maintenance skill for adding resources to this repo consistently. |
| `youtube-analyzer` | Claude Code Plugin | `claude-code/plugins/youtube-analyzer/.claude-plugin/plugin.json` | active | Format-aware YouTube video analysis plugin for Claude Code. |

## Status labels

- `active`: intended for regular use.
- `experimental`: usable, but API or behavior may change.
- `archived`: retained for record/history, not loaded by default if excluded from `package.json#pi`.
- `example`: scaffold/sample resource.
