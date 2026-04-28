# agentic-utilities

Personal Pi package for collecting, versioning, and sharing Pi extensions, agent skills, prompt templates, and themes.

This repo is structured as a real Pi package: install the repo once, then Pi discovers the resources declared in `package.json#pi`.

## Layout

```text
agentic-utilities/
├── .claude-plugin/         # Claude Code marketplace manifest
├── claude-code/            # Claude Code plugin lane
├── extensions/             # Pi extensions: TypeScript modules loaded by Pi
│   └── <name>/index.ts
├── skills/                 # Agent Skills: each skill owns a SKILL.md
│   └── <name>/SKILL.md
├── prompts/                # Prompt templates, named *.prompt.md
├── themes/                 # Pi theme JSON files
├── docs/                   # Catalog, conventions, publishing notes
└── scripts/                # Local scaffolding and inventory helpers
```

## Install locally

From this repo:

```bash
pi -e .                     # try the package for one Pi run
pi install .                # install globally from this local checkout
pi install -l .             # install into the current project's .pi/settings.json
```

After edits inside a running Pi session, run `/reload`.

## Install from Git once published

```bash
pi install git:github.com/AojdevStudio/agentic-utilities
# or pin a tag
pi install git:github.com/AojdevStudio/agentic-utilities@v0.1.0
```

## Add resources

```bash
npm run new:extension -- my-extension "Short description"
npm run new:skill -- my-skill "Use when ..."
npm run list
```

Manual rules:

- Extensions go in `extensions/<kebab-name>/index.ts`.
- Daily-use skills are global-first: test them under `~/.pi/agent/skills/<kebab-name>/`, then keep shared canonical files in `skills/<kebab-name>/` and symlink the global skill back here.
- Skills go in `skills/<kebab-name>/SKILL.md` and must have matching `name` frontmatter.
- Prompt templates go in `prompts/<name>.prompt.md` so docs like `README.md` are not loaded as prompts.
- Themes go in `themes/<name>.json`.
- Claude Code plugins go in `claude-code/plugins/<name>/` and must be listed in `.claude-plugin/marketplace.json`.
- Runtime npm dependencies used by extensions belong in `dependencies`.
- Pi core packages belong in `peerDependencies`: `@mariozechner/pi-coding-agent`, `@mariozechner/pi-ai`, `@mariozechner/pi-tui`, `typebox`.

## Verify

```bash
npm install
npm run lint
npm run typecheck
npm test
npm run check
npm run pack:dry
pi -e .
```

## Current resources

See [`docs/catalog.md`](docs/catalog.md).
