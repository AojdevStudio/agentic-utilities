# Agent Skills

Each directory here is a portable Agent Skill with a `SKILL.md` file that follows the Agent Skills baseline: `name` and `description` frontmatter, with `name` matching the directory.

This directory is the repo's generic skills lane. The skills CLI also discovers skill bundles inside `claude-code/plugins/*/skills/` when pointed at the repository root. For isolated daily use, copy skills into harness-specific inventories (`~/.pi/agent/skills`, `~/.codex/skills`, or `~/.claude/skills`) instead of using a shared `~/.agents` bridge when Pi, Codex, and Claude Code should stay isolated.

## Skills

| Skill | Path | Purpose | Notes |
| --- | --- | --- | --- |
| `adversarial-review` | [`adversarial-review/`](adversarial-review/) | Deep implementation review for bug hunting, audits, stress tests, and ship-readiness checks. | Generic Agent Skill; also available as a Claude Code plugin. |
| `art` | [`art/`](art/) | Creates visual assets, diagrams, infographics, thumbnails, icons, comics, and art direction. | Generic Agent Skill. |
| `awesome-readme` | [`awesome-readme/`](awesome-readme/) | Creates, improves, and reviews GitHub README files with a story-first structure. | Generic Agent Skill; also available as a Claude Code plugin. |
| `bambu-slicer` | [`bambu-slicer/`](bambu-slicer/) | End-to-end Bambu Lab 3D-printing workflow: model discovery, OpenSCAD design, slicing, plates, and printer control. | Generic Agent Skill; also available as a Claude Code plugin. |
| `deep-dive` | [`deep-dive/`](deep-dive/) | Structured technical, operational, and strategic deep-dive analysis. | Generic Agent Skill; also available as a Claude Code plugin. |
| `find-docs` | [`find-docs/`](find-docs/) | Retrieves authoritative current docs, API references, setup notes, and examples before answering or coding. | Generic Agent Skill. |
| `gitworkflow` | [`gitworkflow/`](gitworkflow/) | Git Flow branching, commits, PRs, CI monitoring, merges, releases, submodules, and issue routing. | Generic Agent Skill; also available as a Claude Code plugin. |
| `grill-me` | [`grill-me/`](grill-me/) | Stress-tests plans and designs through a rigorous user interview. | Generic Agent Skill. |
| `grill-with-docs` | [`grill-with-docs/`](grill-with-docs/) | Stress-tests plans against project domain language and records resolved terms/ADRs as decisions crystallize. | Generic Agent Skill. |
| `harness-audit` | [`harness-audit/`](harness-audit/) | Audits repos for autonomous-agent harness readiness and unattended ticket execution gaps. | Generic Agent Skill; also available as a Claude Code plugin. |
| `harness-worktrees` | [`harness-worktrees/`](harness-worktrees/) | Manages Pi/Superconductor worktree refresh and reset workflows after PR merges. | Generic Agent Skill; also available as a Claude Code plugin. |
| `herdr-fleet` | [`herdr-fleet/`](herdr-fleet/) | Launches and reconciles user-confirmed, project-scoped Herdr worker fleets from one control pane. | Requires `HERDR_ENV=1`; defaults to report-only merge policy. |
| `scaffold-notes` | [`scaffold-notes/`](scaffold-notes/) | Maintains this repo's Pi package resources and docs when adding or refactoring skills/extensions/prompts/themes. | Repo maintenance skill. |

## Validate

```bash
bun run validate:skills
bunx skills add . --list
```
