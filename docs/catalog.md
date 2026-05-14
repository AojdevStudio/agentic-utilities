# Resource Catalog

Keep this as the human-readable record of what lives in the package.

| Name | Type | Path | Status | Purpose |
| --- | --- | --- | --- | --- |
| `agentic-utilities` | Claude Code Marketplace | `.claude-plugin/marketplace.json` | active | Marketplace manifest exposing Claude Code plugins from this repo. |
| `adversarial-review` | Extension | `extensions/adversarial-review.ts` | active | Runs adversarial implementation review workflows from Pi as extension tools/commands. |
| `anti-hedging` | Extension | `extensions/anti-hedging.ts` | active | Injects concise anti-hedging response guidance for sharper agent answers. |
| `autopilot` | Extension | `extensions/autopilot/index.ts` | active | Autopilot workflow extension with approvals, preferences, continuation manifests, v2 workflow support, and command docs in `docs/autopilot.md`. |
| `bash-guard` | Extension | `extensions/bash-guard/index.ts` | active | Prompts before risky bash commands, blocks catastrophic subagent commands, and guards sensitive read/write/edit paths with optional JSON policy files. |
| `hello` | Extension | `extensions/hello/index.ts` | example | Smoke-test extension that exposes `/agentic-utilities` and `agentic_utilities_ping`. |
| `interactive-artifacts` | Extension | `extensions/interactive-artifacts/index.ts` | active | Publishes browser-based interactive concept explainer artifacts from Pi. |
| `question` | Extension | `extensions/question.ts` | active | Adds interactive single-question and batch-question UI tools. |
| `todos` | Extension | `extensions/todos.ts` | active | Manages file-backed todos for agent task tracking. |
| `web-tools` | Extension | `extensions/web-tools.ts` | active | Adds web search, fetch, and related browser-grade research tools. |
| `adversarial-review` | Skill | `skills/adversarial-review/SKILL.md` | active | Deep implementation review skill for bug hunting and ship-readiness checks. |
| `art` | Skill | `skills/art/SKILL.md` | active | Creates visual assets, diagrams, infographics, thumbnails, icons, comics, and art direction. |
| `awesome-readme` | Skill | `skills/awesome-readme/SKILL.md` | active | Creates, improves, and reviews GitHub README files with a story-first structure. |
| `bambu-slicer` | Skill | `skills/bambu-slicer/SKILL.md` | active | Public-safe Agent Skill for Bambu Lab 3D-printing workflows: OpenSCAD design, MakerWorld browsing, OrcaSlicer CLI slicing, plate arrangement, and printer control. |
| `deep-dive` | Skill | `skills/deep-dive/SKILL.md` | active | Produces structured deep-dive technical, operational, and strategic analysis. |
| `find-docs` | Skill | `skills/find-docs/SKILL.md` | active | Retrieves authoritative, current technical docs and API references before answering or coding. |
| `gitworkflow` | Skill | `skills/gitworkflow/SKILL.md` | active | Handles Git Flow branching, CI monitoring, PRs, merges, releases, submodules, and issue routing. |
| `grill-me` | Skill | `skills/grill-me/SKILL.md` | active | Stress-tests a plan or design through a rigorous user interview. |
| `harness-audit` | Skill | `skills/harness-audit/SKILL.md` | active | Global-first skill with `~/.pi/agent/skills/harness-audit` symlinked here; audits repo harness readiness and fix gaps. |
| `harness-worktrees` | Skill | `skills/harness-worktrees/SKILL.md` | active | Manages Pi/Superconductor worktree refreshes and resets after PR merges. |
| `scaffold-notes` | Skill | `skills/scaffold-notes/SKILL.md` | active | Maintenance skill for adding resources to this repo consistently. |
| `critical-bug-hunt.prompt` | Prompt | `prompts/critical-bug-hunt.prompt.md` | active | Recent-commit audit prompt for high-severity correctness bugs and minimal fixes. |
| `youtube-analyzer` | Claude Code Plugin | `claude-code/plugins/youtube-analyzer/.claude-plugin/plugin.json` | active | Format-aware YouTube video analysis plugin for Claude Code. |
| `awesome-readme` | Claude Code Plugin | `claude-code/plugins/awesome-readme/.claude-plugin/plugin.json` | active | Story/Utility/Hybrid README generator. Routes diagrams through the `gpt-image-2` skill (Codex CLI + ChatGPT Plus) by default; documents Gemini-API fallback and text-only mode for users without that backend. |

## Status labels

- `active`: intended for regular use.
- `experimental`: usable, but API or behavior may change.
- `archived`: retained for record/history, not loaded by default if excluded from `package.json#pi`.
- `example`: scaffold/sample resource.
