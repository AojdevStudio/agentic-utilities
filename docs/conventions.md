# Conventions

## Package manifest

`package.json#pi` is the source of truth for what Pi loads:

```json
{
  "pi": {
    "extensions": ["./extensions/*.ts", "./extensions/*/index.ts"],
    "skills": ["./skills/**/SKILL.md"],
    "prompts": ["./prompts/*.prompt.md"],
    "themes": ["./themes/*.json"]
  }
}
```

The prompt glob intentionally uses `*.prompt.md` so documentation files in `prompts/` are not loaded as prompt commands.

## Extensions

Preferred shape:

```text
extensions/<name>/
├── index.ts
└── README.md
```

Use a directory when the extension may grow. Single-file extensions can still live at `extensions/<name>.ts`.

Extension checklist:

- Export `default function (pi: ExtensionAPI)`.
- Keep commands clearly named; avoid collisions with built-ins.
- Keep tool names snake_case and package-scoped.
- Add `promptSnippet` and `promptGuidelines` only when the tool should be visible to the model by default.
- Check `ctx.hasUI` before depending on interactive UI behavior.
- Persist branch-sensitive state in tool-result `details`, not only in memory.

## Skills

Preferred shape:

```text
skills/<name>/
├── SKILL.md
├── references/
├── scripts/
└── assets/
```

Skill checklist:

- Canonical repo skills live in `skills/<name>/` and are validated with `npm run validate:skills`.
- Some skills intentionally appear in multiple distribution lanes, such as root Agent Skills and bundled Claude Code plugin skills. `npm run validate:skills` warns on these duplicates so drift stays visible, but expected distribution-lane duplicates are not failures.
- Install or copy daily-use skills into harness-specific locations: `~/.pi/agent/skills`, `~/.codex/skills`, or `~/.claude/skills`.
- Prefer copy mode when Pi, Codex, and Claude Code should diverge; avoid `~/.agents` as the default shared bridge. If using the external `skills` CLI as an installer, verify its output first because some agent targets can still resolve to shared Agent Skills paths.
- Use symlinks only when you intentionally want coupled inventories and have documented the source of truth.
- Frontmatter `name` exactly matches `<name>`.
- `description` says when to use it, not just what it is, and stays within Agent Skills metadata limits.
- Relative links/scripts resolve from the skill directory.
- Keep long reference material outside `SKILL.md` and link to it.

## Prompts

Use `prompts/<name>.prompt.md`. Keep prompts short and task-specific.

## Claude Code plugins

- Marketplace metadata lives in `.claude-plugin/marketplace.json`.
- Plugin source lives in `claude-code/plugins/<name>/`.
- Plugin names use kebab-case.
- Verify GitHub owner/repo URLs with `gh repo view` or `git remote -v` before writing manifests.
- Replace personal absolute paths with `${CLAUDE_PLUGIN_ROOT}` for bundled files or interactive configuration for user-owned output locations.

## Themes

Use `themes/<name>.json`. Include screenshots or notes in `docs/` rather than inside `themes/`.

## CodeGraph

This repo tracks an initialized CodeGraph snapshot in `.codegraph/codegraph.db` so agents can start with structural code context immediately. When the CodeGraph MCP daemon is active, its file watcher auto-syncs the graph on source changes. Before committing changes that should affect the graph, run `codegraph status .` and make sure the index is up to date.

Volatile sidecars (`*.db-wal`, `*.db-shm`, locks, logs, and cache files) stay ignored under `.codegraph/`.

## Local quality gates

- `npm test` runs behavioral smoke tests.
- `npm run lint` runs Biome linting.
- `npm run typecheck` runs TypeScript checks.
- `npm run validate:skills` checks `skills/**/SKILL.md` frontmatter against the Agent Skills baseline used by the skills CLI.
- `npm run check` runs lint, typecheck, tests, plugin validation, skill validation, and resource inventory.
- `codegraph status .` confirms the tracked CodeGraph snapshot is current.
- Husky + lint-staged run Biome on staged files before commit, then typecheck and smoke tests.
