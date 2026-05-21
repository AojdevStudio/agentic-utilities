---
name: skill-stats
description: "Report Claude Code skill-usage telemetry — top used, recently active, dormant skills, plus on-disk size. USE WHEN: skill stats, skill usage, dormant skills, which skills do I use, prune skills, skill telemetry, what skills are dormant, most popular skills."
---

# skill-stats — Usage & Dormancy Report

Aggregates skill-invocation telemetry written by this plugin's bundled PreToolUse hook (`${CLAUDE_PLUGIN_ROOT}/scripts/log-skill.ts`) and cross-references against the on-disk skill catalog at `~/.claude/skills/` and `~/.claude/plugins/marketplaces/`.

## How it works

1. **Hook (auto-installed):** When this plugin is enabled, a PreToolUse hook fires on every `Skill` tool invocation and appends one JSONL event to the plugin's telemetry file (`${CLAUDE_PLUGIN_DATA}/events.jsonl`, falling back to `~/.claude/plugins/data/skill-stats/events.jsonl`). The hook is non-blocking — any failure exits silently and never interferes with skill execution.
2. **Report (this skill):** The reporting script reads that telemetry, scans installed skills, and produces a usage report.

**First-run note:** Right after install, telemetry is empty. Use the plugin for a few days/sessions to accumulate data, then run the report.

## How to use

Run the underlying script directly:

```bash
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts                    # full text report
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --json              # machine-readable JSON
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --days=7            # restrict to last N days
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --stale=14          # custom stale threshold
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --author=NAME       # filter by SKILL.md frontmatter `authors:`
```

The `--author=NAME` view reads the `authors:` field from each SKILL.md frontmatter and lists matching skills with their usage counts, session counts, last-used age, and on-disk size. Dormant authored skills surface with `count = 0` for easy pruning decisions.

When invoking this skill in chat, run the JSON form, parse it, and present a clean markdown table with these sections:

1. **Headline** — installed total, distinct used, dormant, phantom counts; telemetry window
2. **Top 15 most used** — skill name, count, % of total, last used
3. **Dormant (top 20 by disk size)** — skill name, source (user/plugin), size, last touched
4. **Phantom** — names appearing in telemetry but not on disk (usually plugin-namespaced commands or uninstalled skills)
5. **Pruning candidates** — total disk that would be reclaimed by removing all dormant skills

## Caveats to surface in the report

- The telemetry window only goes back to when the plugin's hook started recording. A "dormant" skill may simply be one used before the plugin was installed.
- Plugin-namespaced names like `codex:rescue` show as phantom because they are slash-commands, not standalone SKILL.md files. They are real and used, just stored differently.
- Symlinked skill directories are followed by realpath so the same physical directory linked under multiple paths is counted once.

## Snapshot history

Snapshots are useful for trend comparison. To save one:

```bash
mkdir -p "${CLAUDE_PLUGIN_DATA}/snapshots"
bun ${CLAUDE_PLUGIN_ROOT}/scripts/report.ts --json > "${CLAUDE_PLUGIN_DATA}/snapshots/skill-stats-$(date +%Y%m%d).json"
```

If `${CLAUDE_PLUGIN_DATA}` is not set in your shell, fall back to `~/.claude/plugins/data/skill-stats/snapshots/`.

## Telemetry schema

Each line of `events.jsonl` is a self-contained JSON object:

```json
{"event":"skill_invocation","skill":"deep-dive","session_id":"abc123","timestamp":"2026-05-01T20:00:00.000Z"}
```

The hook never logs tool inputs, file paths, or user prompts — only the skill name, session ID, and timestamp. Sessions named `smoke` or `test` are excluded.
