# Harness Skills

## Rules

- Treat daily-use skills as harness-isolated resources by default.
- Keep canonical shared skills in `skills/<name>/` so this package can validate, document, and distribute them.
- Copy a canonical skill into the harness inventory that should use it: `~/.pi/agent/skills/<name>/`, `~/.codex/skills/<name>/`, or `~/.claude/skills/<name>/`.
- Prefer copy mode when Pi, Codex, and Claude Code should diverge in tools, prompts, paths, model assumptions, or workflow details.
- Use symlinks only when you intentionally want coupled behavior between a harness inventory and the canonical repo skill.
- When using a symlink, document the source of truth in the skill or nearby docs so future edits do not fork behavior accidentally.
- Avoid a shared `~/.agents` bridge as the default install path because it hides which harness owns which behavior.
- Preserve Agent Skills structure: `skills/<name>/SKILL.md`, frontmatter `name` matching the directory, and a precise trigger-oriented `description`.
- Treat duplicate skill-name warnings across root skills and Claude Code plugin skills as expected distribution-lane signal, not validation failure. Investigate them when the duplicate was not intentional or when one lane drifts semantically from the other.

## Rationale

Pi, Codex, and Claude Code often need different tool names, filesystem assumptions, prompts, and model behavior. The repo exists to share canonical resources, but daily harness inventories should be allowed to diverge deliberately. Copies make divergence explicit; symlinks are reserved for the narrower case where coupled behavior is desired.
