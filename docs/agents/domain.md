# Project Context Docs

How engineering and design skills should consume this repo's project context before exploring the codebase or changing UI.

## Layout

This is a single-context repo.

- **Domain glossary:** `CONTEXT.md` at the repo root.
- **Architecture decisions:** `docs/adr/`.
- **Product strategy:** no `PRODUCT.md` is configured.
- **Design system:** no `DESIGN.md` is configured.

## Before exploring, read these

- Read `CONTEXT.md` before naming domain concepts in issue titles, plans, refactors, tests, or documentation.
- Read relevant ADRs in `docs/adr/` before making architecture or workflow decisions.
- For Pi extension work, also read `rules/extensions.md` and `rules/package-resources.md`.
- For package resource changes, update `docs/catalog.md` in the same change.

## Product and design work

This repo is primarily a package of Pi extensions, skills, prompts, themes, and plugins. If future work becomes product, UI, UX, copy, onboarding, or visual design work, create or request the relevant `PRODUCT.md` / `DESIGN.md` context first instead of inventing product/design assumptions.

## Use the glossary's vocabulary

When output names a domain concept, use the term as defined in `CONTEXT.md`. Do not drift to synonyms the glossary explicitly avoids.

If the concept you need is missing, either reconsider whether it belongs to this repo's language or note it for `/grill-with-docs`.

## Flag ADR conflicts

If output contradicts an existing ADR, surface it explicitly rather than silently overriding it.
