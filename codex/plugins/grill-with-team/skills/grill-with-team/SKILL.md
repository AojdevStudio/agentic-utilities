---
name: grill-with-team
description: Codex-native team grilling session that stress-tests a plan against the existing codebase and domain model, sharpens terminology, and maintains `CONTEXT.html` plus `docs/adr/*.html`. Use when the user wants to grill a plan with the team, produce or update a visual HTML domain context, migrate `CONTEXT.md`/ADRs to HTML, or run a rigorous architecture/product planning interview before implementation.
---

# Grill With Team

Run a Codex-native team grill. The goal is to turn a fuzzy plan into precise domain language, resolved/open decision state, and sparse ADRs where decisions are genuinely worth recording.

## Hard Boundary

- This plugin is Codex-owned. Do not edit Pi/autopilot configuration, Claude Code plugin files, or global skill roots.
- This workflow requires Codex subagent support. If subagent tools are unavailable, stop and tell the user this plugin requires a Codex session with subagents.
- Use the bundled companion skills in this plugin when the user asks for their workflows: `prototype`, `setup-matt-pocock-skills`, `to-prd`, `to-issues`, `triage`, `diagnose`, `tdd`, and `zoom-out`.

## Artifact Contract

- `CONTEXT.html` is the canonical glossary, visual domain map, and decision-state artifact. Use [../../references/context-html-format.md](../../references/context-html-format.md) and [../../assets/context-template.html](../../assets/context-template.html).
- `docs/adr/*.html` holds sparse, one-decision-per-file ADRs. Use [../../references/adr-html-format.md](../../references/adr-html-format.md).
- Within a migrated repo, do not recreate `CONTEXT.md` or markdown ADRs.
- Edit HTML surgically: one element per line, one shared `<style>` block, no wholesale regeneration for small content changes.

## Workflow

### Phase 0: Setup Or Migration

1. Inspect the repo for `CONTEXT.html`, `CONTEXT.md`, `CONTEXT-MAP.html`, `CONTEXT-MAP.md`, and `docs/adr/`.
2. If markdown context exists and `CONTEXT.html` does not, follow [../../references/migration.md](../../references/migration.md).
3. Before deleting markdown, run the verifier:

```bash
python3 ${CODEX_PLUGIN_ROOT:-$HOME/plugins/grill-with-team}/scripts/verify_html_migration.py \
  --context-md CONTEXT.md \
  --context-html CONTEXT.html \
  --adr-md-dir docs/adr \
  --adr-html-dir docs/adr
```

4. If no context artifact exists, create `CONTEXT.html` lazily from the template after the first term or decision resolves.

### Phase 1: Team Prep

Spawn the prep team as Codex subagents and keep their raw output out of the main answer:

- Explorer: use [../../references/roles/explorer.md](../../references/roles/explorer.md).
- Architect: use [../../references/roles/architect.md](../../references/roles/architect.md).
- Researcher: use [../../references/roles/researcher.md](../../references/roles/researcher.md) only when the plan depends on current APIs, external patterns, or domain prior art.

Synthesize their results into a working decision ledger in conversation. The ledger is not `CONTEXT.html`; only durable glossary terms and resolved/open decision state go into the artifact.

### Phase 2: Grill

1. Walk the decision tree one material branch at a time.
2. Before each major decision question, spawn RedTeam/Cato using [../../references/roles/redteam-cato.md](../../references/roles/redteam-cato.md).
3. Apply the question gate in [../../references/question-discipline.md](../../references/question-discipline.md).
4. Ask one recommendation-first question at a time. Wait for the user before moving to the next branch.
5. As terms and decisions resolve, update `CONTEXT.html` surgically.

### Phase 3: Finalize

1. Spawn Designer using [../../references/roles/designer.md](../../references/roles/designer.md) to improve domain-map readability without changing decided meaning.
2. Offer HTML ADRs only when the ADR-worthiness test in [../../references/question-discipline.md](../../references/question-discipline.md) passes.
3. Report the resulting artifacts and verification status.

## Companion Skill Routing

Use [../../references/bundled-skills.md](../../references/bundled-skills.md) to choose the bundled companion skill when the user is not asking for a full grill.
