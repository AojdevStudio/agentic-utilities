# Harness Garbage Collection

Run this review weekly or after a cluster of agent failures.

## Goal

Convert repeated friction into permanent harness assets: rules, lints, docs, prompts, skills, tests, or extension checks.

## Checklist

1. Review recent PR comments, failed CI runs, failed `npm run check` output, and agent handoff notes.
2. Group failures by repeated cause, not by individual symptom.
3. For each repeated cause, choose the smallest durable fix:
   - Rule gap → add or update `rules/*.md` and link it from `AGENTS.md`.
   - Mechanical style drift → update `biome.json` or lint-staged config.
   - Test blind spot → add or wire a test into `npm test` or `npm run check`.
   - Resource discovery gap → update `package.json#pi`, `docs/catalog.md`, or scaffolding scripts.
   - Workflow confusion → update `AGENTS.md`, a skill, or a prompt template.
4. Keep changes surgical. Do not mix unrelated cleanup with harness GC.
5. Run `npm run check`, `npm run pack:dry`, and `pi -e .` after changes.

## Cadence

Default: Friday afternoon local time, or immediately after two similar agent failures in one week.

## Output

Create a small PR or commit with:

- The repeated failure pattern.
- The durable harness fix.
- Verification output.
- Any remaining follow-up as an issue or TODO entry.
