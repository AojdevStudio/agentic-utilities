# skill-stats

Telemetry-driven report on Claude Code skill usage. Tells you which skills you actually use, which ones are gathering dust, and how much disk they cost.

## What it does

When enabled, this plugin:

1. **Bundles a PreToolUse hook** (`hooks/log-skill.ts`) that fires on every `Skill` tool invocation and appends one JSONL event to plugin-owned telemetry. The hook is non-blocking — failures exit silently and never interfere with skill execution.
2. **Provides a report skill** that reads that telemetry, scans installed skills (`~/.claude/skills/`, `~/.claude/plugins/marketplaces/`), and generates either a human-readable text report or JSON.

## Storage layout

Telemetry lives at `${CLAUDE_PLUGIN_DATA}/events.jsonl` — Claude Code's canonical plugin-scoped data path, which resolves to `~/.claude/plugins/data/skill-stats/events.jsonl` and survives plugin updates. If `CLAUDE_PLUGIN_DATA` is not exported (e.g., when running the report manually outside a plugin context), the script falls back to the same `~/.claude/plugins/data/skill-stats/events.jsonl` path.

The hook never logs tool inputs, file contents, or user prompts. Only:

```json
{"event":"skill_invocation","skill":"<name>","session_id":"<id>","timestamp":"<iso>"}
```

## Trigger phrases

Auto-activates when you ask:

- "show me skill stats"
- "which skills are dormant?"
- "what skills do I use most?"
- "skill telemetry"
- "prune unused skills"
- "skill usage report"

Or run the report directly:

```bash
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts          # text report
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --json   # JSON
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --days=7
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --author=YourName
```

## Prerequisites

- **Bun** — the hook and report scripts run on Bun (`curl -fsSL https://bun.sh/install | bash`).
- **Time** — telemetry only covers events captured after install. Expect 3–7 days of normal usage before the report has signal.

## Coexisting with other skill-tracking hooks

Plugin hooks merge with user-level hooks in parallel. If you already maintain your own skill-tracking PreToolUse hook (e.g., a personal `SkillGuard.hook.ts`), this plugin's hook runs alongside it without interfering — they write to different files and never coordinate.

## License

MIT — see repository LICENSE.
