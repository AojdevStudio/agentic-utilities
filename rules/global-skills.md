# Global Skills

## Rules

- Treat daily-use skills as global-first resources.
- Import and test a new daily-use skill under `~/.pi/agent/skills/<name>/` first.
- When sharing that skill from this repo, keep the canonical files in `skills/<name>/` and symlink `~/.pi/agent/skills/<name>` back to the repo path.
- Do not maintain separate global and repo copies. Copies drift silently.
- Preserve Agent Skills structure: `skills/<name>/SKILL.md`, frontmatter `name` matching the directory, and a precise trigger-oriented `description`.

## Rationale

Global skills are what Pi loads during normal daily work. The repo exists to share and version those resources, not to fork a second copy that can diverge from the global one.
