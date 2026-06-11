# grill-with-team

Codex-native plugin for rigorous domain grilling and the companion engineering skills that support the workflow.

## What it includes

- `grill-with-team` — main Codex-native team grilling workflow.
- `prototype` — throwaway logic or UI prototypes for design exploration.
- `setup-matt-pocock-skills` — per-repo agent workflow setup.
- `to-prd` — synthesize the current context into a tracker-backed PRD.
- `to-issues` — break plans into independently grabbable vertical slices.
- `triage` — move issues through a small triage role state machine.
- `diagnose` — disciplined bug and regression diagnosis.
- `tdd` — red-green-refactor development discipline.
- `zoom-out` — higher-level module and caller maps.

## Contract

`grill-with-team` maintains one canonical domain artifact:

```text
CONTEXT.html
docs/adr/*.html
```

The workflow uses Codex subagents for Explorer, Architect, Researcher, RedTeam/Cato, and Designer passes. It does not modify Pi/autopilot configuration and does not mutate Claude or Pi skill roots.

## Install from this checkout

This repo is the source of truth. Symlink it into the personal Codex plugin directory:

```bash
mkdir -p ~/plugins
ln -sfn "$(pwd)/codex/plugins/grill-with-team" ~/plugins/grill-with-team
codex plugin add grill-with-team@personal
```

## Verify

```bash
python3 ~/plugins/grill-with-team/scripts/verify_fixtures.py
```

The verifier checks that markdown context terms and ADR identifiers survive the agent-led HTML migration.
