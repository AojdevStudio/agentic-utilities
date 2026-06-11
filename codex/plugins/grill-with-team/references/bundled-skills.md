# Bundled Skill Routing

This plugin bundles the core team grill plus companion skills. Use the smallest skill that matches the user's request.

| Request shape | Use |
| --- | --- |
| Stress-test a plan, sharpen domain language, migrate/update `CONTEXT.html`, or create sparse ADRs | `grill-with-team` |
| Configure repo workflow docs so agent skills know the tracker, project board, labels, and context docs | `setup-matt-pocock-skills` |
| Turn conversation context into a PRD | `to-prd` |
| Break a PRD/plan/spec into tracker issues | `to-issues` |
| Triage incoming issues or prepare a specific issue for an agent | `triage` |
| Diagnose a bug, failing behavior, or performance regression | `diagnose` |
| Build or fix behavior with red-green-refactor discipline | `tdd` |
| Build a throwaway logic or UI prototype | `prototype` |
| Explain a code area from a higher level before diving in | `zoom-out` |

Do not reach into global Pi, Claude, or Codex skill roots. The plugin-local copies are the packaged source for this Codex workflow.
