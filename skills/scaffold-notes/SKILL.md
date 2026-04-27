---
name: scaffold-notes
description: Maintains the agentic-utilities Pi package. Use when adding, documenting, inventorying, or refactoring Pi extensions, Agent Skills, prompt templates, or themes in this repository.
---

# Scaffold Notes

Use this skill when changing this repository's resource inventory.

## Add an extension

1. Create `extensions/<kebab-name>/index.ts`.
2. Export `default function (pi: ExtensionAPI)`.
3. Document commands, tools, events, and dependencies in `extensions/<kebab-name>/README.md`.
4. Add an entry to `docs/catalog.md`.
5. Run `npm run check` and `pi -e .`.

## Add a skill

1. Create `skills/<kebab-name>/SKILL.md`.
2. Ensure frontmatter `name` matches the directory exactly.
3. Write a description that says when to use the skill.
4. Put long references under `references/` and helper code under `scripts/` inside the skill directory.
5. Add an entry to `docs/catalog.md`.
6. Run `npm run list` and `pi -e .`.

## Add prompts or themes

- Prompts: `prompts/<name>.prompt.md`.
- Themes: `themes/<name>.json`.
- Update `docs/catalog.md` for anything intended for reuse.

## Safety

Never commit secrets or local session state. Runtime dependencies belong in `dependencies`; Pi core packages belong in `peerDependencies`.
