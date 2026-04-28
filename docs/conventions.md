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

- Daily-use skills are global-first: test under `~/.pi/agent/skills/<name>/` first.
- For shared daily-use skills, keep canonical files in `skills/<name>/` and symlink the global skill back to that repo path.
- Frontmatter `name` exactly matches `<name>`.
- `description` says when to use it, not just what it is.
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

## Local quality gates

- `npm test` runs behavioral smoke tests.
- `npm run lint` runs Biome linting.
- `npm run typecheck` runs TypeScript checks.
- `npm run check` runs lint, typecheck, tests, and resource inventory.
- Husky + lint-staged run Biome on staged files before commit, then typecheck and smoke tests.
