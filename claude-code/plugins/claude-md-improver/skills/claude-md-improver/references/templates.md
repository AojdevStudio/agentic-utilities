# Instruction File Templates

## Key Principles

- **Lean**: ~100-150 lines for root file. Every line earns its place.
- **Intent over inventory**: Teach *how the project thinks*, not what every file contains.
- **Failure-driven**: Each gotcha traces to a real past problem.
- **Point, don't repeat**: Reference source-of-truth files instead of duplicating content.
- **Zero staleness risk**: No hardcoded counts, no volatile listings.

---

## Template: Project Root (Lean)

The default. Works for most projects.

```markdown
# <Project Name>

<One-line description of what this project does and why it exists.>

**Status:** <Current state — shipped, in development, etc.> See `<planning-file>` for roadmap.

## Stack

<Runtime> | <Framework> | <Language> | <Database> | <Key libs for non-obvious choices only>

## Commands

Core: `<dev>`, `<test>`, `<build>`. See `package.json` scripts for the full list.

## Architecture

<3-5 line description of HOW the project is organized and WHY — not a full tree.>
<Point to docs/ARCHITECTURE.md if it exists.>

## Conventions

- <Convention with rationale — WHY this pattern, not just WHAT>
- <Convention with rationale>
- <Convention with rationale>

## Gotchas

- **<Problem>** — <What goes wrong and how to fix it>
- **<Problem>** — <What goes wrong and how to fix it>

## Boundaries

- <Hard constraint agents must never violate>
- <Hard constraint agents must never violate>
```

---

## Template: Project Root (Comprehensive)

For complex projects needing more context. Still aims for ~150 lines.

```markdown
# <Project Name>

> <One-line description.>

**Status:** <State.> See `<file>` for roadmap.

## Stack

<Stack summary. Non-obvious lib choices only.>

Key libs: `<lib>` (<why>), `<lib>` (<why>)

## Commands

Core: `<dev>`, `<test>`, `<build>`. See `package.json` for full list.

## Architecture

<How the project is organized — intent and constraints, not file inventory.>
<Key directories with PURPOSE, not just names.>

## Conventions

### <Domain 1>
- <Convention> — <because rationale>

### <Domain 2>
- <Convention> — <because rationale>

### <Workflow>
<Non-obvious workflow sequence: step → step → step>

## Security

- <Security boundary or constraint>
- <Key security pattern and why>

## Gotchas

1. **<Problem>** — <context, cause, and fix>
2. **<Problem>** — <context, cause, and fix>

## Key Decisions

- <Decision> — <why we chose this over the alternative>
- <Decision> — <why>

See `<decision-log-file>` for full history.
```

---

## Template: Monorepo Root

```markdown
# <Monorepo Name>

<Description.>

## Packages

| Package | Purpose |
|---------|---------|
| `<name>` | <what it does> |

## Commands

Root: `<install>`, `<dev>`, `<build>`. Per-package commands in each package's CLAUDE.md.

## Cross-Package Patterns

- <Shared convention or dependency rule>
- <Build/deploy ordering>
- <Shared types or generated code patterns>

## Gotchas

- <Monorepo-specific gotcha>
```

---

## Template: Subsystem / Package

Keep these very short — just what's unique to this subsystem.

```markdown
# <Package Name>

<Purpose — what this package does and why it exists as a separate unit.>

## Key Exports

- `<export>` — <purpose>

## Conventions

- <Package-specific pattern that differs from root conventions>

## Gotchas

- <Package-specific gotcha>
```

---

## Template: Layered Docs (For Large Projects)

When a single file exceeds ~200 lines, split into this structure:

```
CLAUDE.md              ← lean index: intent, conventions, gotchas, pointers
docs/
├── ARCHITECTURE.md    ← system design, domain map, data flow
├── CONVENTIONS.md     ← code style, patterns, naming rationale
├── WORKFLOWS.md       ← development workflows, CI/CD, release process
└── references/        ← external docs reformatted for agent consumption
```

The root CLAUDE.md becomes purely navigational:

```markdown
# <Project Name>

> <Description.>

**Status:** <State.>

## Quick Context

<5-line summary of what this project is and how it thinks.>

## Commands

Core: `<dev>`, `<test>`, `<build>`. See `package.json` for all scripts.

## Docs

| Doc | When to read |
|-----|-------------|
| `docs/ARCHITECTURE.md` | Understanding system design or adding features |
| `docs/CONVENTIONS.md` | Writing new code or reviewing patterns |
| `docs/WORKFLOWS.md` | CI/CD, deployment, release process |

## Gotchas

- <Top 3-5 gotchas that cause the most agent failures>

## Boundaries

- <Top constraints that must never be violated>
```
