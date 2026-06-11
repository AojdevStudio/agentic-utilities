# Project Context Docs

How engineering and design skills should consume this repo's project context
before exploring the codebase or changing UI.

## Before exploring, read these

- **`CONTEXT.md`** (or **`CONTEXT.html`**) at the repo root, or
- **`CONTEXT-MAP.md`** (or **`CONTEXT-MAP.html`**) at the repo root if it exists — it points at one `CONTEXT.md`/`CONTEXT.html` per context. Read each one relevant to the topic.
- **`docs/adr/`** — read ADRs (`*.md` or `*.html`) that touch the area you're about to work in. In multi-context repos, also check `src/<context>/docs/adr/` for context-scoped decisions.

> **Either extension is valid.** Some repos store these docs as HTML (`CONTEXT.html`, `CONTEXT-MAP.html`, `docs/adr/*.html`) instead of Markdown — a repo that has run `grill-with-team` will have the `.html` form and no `.md`. If you find the `.html` variant, treat it as exactly as authoritative as the `.md` would be. Never conclude a doc is missing just because the `.md` isn't there; check the `.html` too.
- **`PRODUCT.md`** — read before UI, UX, product, copy, onboarding, or design-system work. It defines register, users, product purpose, brand personality, anti-references, strategic principles, and accessibility needs.
- **`DESIGN.md`** — read before visual UI work when present. It defines the visual system: theme, colors, typography, elevation, components, and do/don't rules.

`PRODUCT.md` and `DESIGN.md` are resolved case-insensitively using the
`impeccable` convention:

1. `IMPECCABLE_CONTEXT_DIR`, if set
2. repo root
3. `.agents/context/`
4. `docs/`

If `PRODUCT.md` is missing and the task is design work, run or recommend
`impeccable teach` before changing UI. If `DESIGN.md` is missing, proceed once
per session after noting that `impeccable document` can generate it.

If neither `CONTEXT.md`/`CONTEXT.html` nor any ADRs exist, **proceed silently**.
Don't flag their absence; don't suggest creating them upfront. The producer
skills (`grill-with-docs`, or `grill-with-team` for the HTML form) create them
lazily when terms or decisions actually get resolved.

## File structure

Single-context repo (most repos):

```text
/
├── CONTEXT.md
├── PRODUCT.md
├── DESIGN.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

Multi-context repo (presence of `CONTEXT-MAP.md` at the root):

```text
/
├── CONTEXT-MAP.md
├── PRODUCT.md                         ← repo-wide product/design strategy
├── DESIGN.md                          ← repo-wide visual system, if shared
├── docs/adr/                          ← system-wide decisions
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← context-specific decisions
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md` (or `CONTEXT.html`). Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `grill-with-docs`, or `grill-with-team`).

## Use product and design vocabulary for UI work

When your output names a user, workflow, surface, brand attribute, design
principle, anti-reference, color, type role, component, or visual state, use the
term as defined in `PRODUCT.md` or `DESIGN.md`.

`PRODUCT.md` is strategic: who this is for, what it is trying to do, what it
must not feel like, and what principles guide tradeoffs.

`DESIGN.md` is visual and operational. When it follows the Stitch convention,
it opens with token frontmatter and then uses these sections:

1. `## Overview`
2. `## Colors`
3. `## Typography`
4. `## Elevation`
5. `## Components`
6. `## Do's and Don'ts`

Do not duplicate visual rules into `PRODUCT.md`. Do not overwrite an existing
`DESIGN.md` without user confirmation; refresh it through `impeccable document`
when the visual system has drifted.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
