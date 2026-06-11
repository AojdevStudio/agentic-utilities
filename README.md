<div align="center">

# agentic-utilities

**A harness-agnostic library of Pi extensions, Claude Code plugins, Agent Skills, prompts, and themes — installable as one Pi package or as a Claude Code marketplace.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#contributing)
[![Pi Package](https://img.shields.io/badge/pi-package-blue.svg)](https://github.com/mariozechner/pi)
[![Claude Code Marketplace](https://img.shields.io/badge/claude--code-marketplace-8a2be2.svg)](.claude-plugin/marketplace.json)
[![GitHub stars](https://img.shields.io/github/stars/AojdevStudio/agentic-utilities?style=social)](https://github.com/AojdevStudio/agentic-utilities/stargazers)

</div>

---

## What's in the box

| Resource | Type | Purpose |
| --- | --- | --- |
| [`autopilot`](extensions/autopilot/) | Pi Extension | Approval-gated workflow runner with continuation manifests and v2 flow. |
| [`conditional-hooks`](extensions/conditional-hooks/) | Pi Extension | Loads explicit JSON hook policy, including the documented worktree-GC-on-merge example. |
| [`bambu-slicer`](skills/bambu-slicer/) · [plugin](claude-code/plugins/bambu-slicer/) | Skill + Claude Code plugin | End-to-end Bambu Lab pipeline: OpenSCAD design, MakerWorld browsing, OrcaSlicer-backed STL→3MF, plate arrangement, printer control. |
| [`harness-audit`](skills/harness-audit/) | Skill (global-first) | Audits a repo for AI-harness readiness across the 10-artifact stack and dispatches surgical fixes. |
| [`scaffold-notes`](skills/scaffold-notes/) | Skill | Maintenance helper for adding resources to this repo consistently. |
| [`youtube-analyzer`](claude-code/plugins/youtube-analyzer/) | Claude Code plugin | Format-aware YouTube video analysis with multi-agent transcript chunking. |
| [`critical-bug-hunt`](prompts/critical-bug-hunt.prompt.md) | Prompt template | Recent-commit audit for high-severity correctness bugs and minimal fixes. |
| [`hello`](extensions/hello/) | Example extension | Smoke-test scaffold exposing `/agentic-utilities` and `agentic_utilities_ping`. |

The full catalog (with statuses) lives in [`docs/catalog.md`](docs/catalog.md).

## Quick start

### As a Pi package

```bash
pi -e .                    # try the package for one Pi run
pi install .               # install globally from this checkout
pi install -l .            # install into the current project's .pi/settings.json
```

After edits inside a running Pi session, run `/reload`.

For Conditional Hooks, put global config in `~/.pi/agent/conditional-hooks.json` or trusted project config in `.pi/conditional-hooks.json`; see [`extensions/conditional-hooks/README.md`](extensions/conditional-hooks/README.md).

Once published:

```bash
pi install git:github.com/AojdevStudio/agentic-utilities
pi install git:github.com/AojdevStudio/agentic-utilities@v0.1.0
```

### As a Claude Code marketplace

Add the marketplace, then install plugins from it:

```bash
/plugin marketplace add AojdevStudio/agentic-utilities
/plugin install bambu-slicer@agentic-utilities
/plugin install youtube-analyzer@agentic-utilities
```

The marketplace manifest is [`.claude-plugin/marketplace.json`](.claude-plugin/marketplace.json).

## Layout

```text
agentic-utilities/
├── .claude-plugin/         # Claude Code marketplace manifest
├── claude-code/plugins/    # Claude Code plugins (bambu-slicer, youtube-analyzer)
├── extensions/             # Pi extensions: TypeScript modules loaded by Pi
│   └── <name>/index.ts
├── skills/                 # Agent Skills: each skill owns a SKILL.md
│   └── <name>/SKILL.md
├── prompts/                # Prompt templates, named *.prompt.md
├── themes/                 # Pi theme JSON files
├── rules/                  # Authoritative rule files (extensions, skills, identity, secrets)
├── docs/                   # Catalog, conventions, autopilot reference, GC ritual
└── scripts/                # Local scaffolding (new-extension, new-skill, list-resources)
```

## Add a resource

```bash
npm run new:extension -- my-extension "Short description"
npm run new:skill -- my-skill "Use when ..."
npm run list                                # inventory check
```

Conventions in one breath:

- **Extensions** → `extensions/<kebab-name>/index.ts`. Tool names use snake_case and are globally namespaced (e.g. `agentic_utilities_ping`).
- **Skills** → `skills/<kebab-name>/SKILL.md`, frontmatter `name` matches the directory. Daily-use skills are global-first: develop under `~/.pi/agent/skills/<name>/`, then keep canonical files here and symlink the global skill back so Pi loads it globally and `npm pack` ships it.
- **Prompts** → `prompts/<name>.prompt.md` (the `.prompt.md` suffix keeps READMEs out of the prompt list).
- **Themes** → `themes/<name>.json`.
- **Claude Code plugins** → `claude-code/plugins/<name>/`, registered in [`.claude-plugin/marketplace.json`](.claude-plugin/marketplace.json).

Full conventions: [`docs/conventions.md`](docs/conventions.md). Authoritative rule files: [`rules/`](rules/).

## Verify before shipping

```bash
npm install
npm run lint
npm run typecheck
npm test
npm run check         # bundles lint + typecheck + tests
npm run pack:dry      # confirm tarball contents
pi -e .               # smoke-test the package
```

A gitleaks pre-commit hook plus a CI `secret-scan` job block accidental secret/PII commits — see the "Secret + PII gate" section in [`AGENTS.md`](AGENTS.md) for how to extend it with personal terms.

## Project docs

| Doc | What it covers |
| --- | --- |
| [`docs/autopilot.md`](docs/autopilot.md) | `/autopilot` command reference and the common v2 flow. |
| [`docs/catalog.md`](docs/catalog.md) | Live resource inventory (canonical source for "what's in the box"). |
| [`docs/conventions.md`](docs/conventions.md) | Repo conventions for extensions, skills, prompts, themes, and plugins. |
| [`docs/publishing.md`](docs/publishing.md) | Publishing notes. |
| [`docs/garbage-collection.md`](docs/garbage-collection.md) | Weekly / post-failure GC ritual that converts agent friction into rules, lints, and skills. |

## Compatibility

- **Pi runtime** — declared via `peerDependencies`: `@mariozechner/pi-coding-agent`, `@mariozechner/pi-ai`, `@mariozechner/pi-tui`, `typebox`.
- **Claude Code** — plugins follow the [Claude Code marketplace schema](https://anthropic.com/claude-code/marketplace.schema.json).
- **Node** — runtime npm dependencies used by extensions belong in `dependencies`; pin sparingly.

## Source code reference

`opensrc/sources.json` lists fetched dependency source trees. To add another:

```bash
npx opensrc <package>            # npm
npx opensrc pypi:<package>       # PyPI
npx opensrc crates:<package>     # crates.io
npx opensrc <owner>/<repo>       # GitHub
```

## Contributing

PRs welcome. Before opening one:

1. Add or update an entry in [`docs/catalog.md`](docs/catalog.md).
2. Run `npm run check` and `npm run pack:dry`.
3. Run `pi -e .` to smoke-test against a real Pi session.
4. Identity-bearing values (GitHub slugs, package names, versions) must be verified against the source of truth — see [`rules/identity-verification.md`](rules/identity-verification.md).

## License

[MIT](LICENSE) © Ossie Irondi.

---

<div align="center">

If a skill, extension, or plugin saved you an afternoon, **[star the repo](https://github.com/AojdevStudio/agentic-utilities/stargazers)** so the next person finds it too.

</div>
