# repo-architect

Repository organization expert — structure, audit, and refactor via 8 framework archetypes. Encodes the Repo Architect blueprint: archetype selection, structured intake, health checks, CI guardrail templates, and move-map migration planning, so every repo gets intentional structure rather than ad-hoc folder creation.

## What it does

The skill routes a request to one of 4 workflow playbooks:

| Workflow | Trigger | What it produces |
|----------|---------|------------------|
| NewProject | "new project", "scaffold project" | Canonical tree, CI config, implementation steps |
| AuditRepo | "audit repo", "repo health", "check structure" | Structure score + prioritized fix list |
| RefactorPlan | "refactor repo", "restructure", "move map" | Phased Move Map with rollback |
| ExecuteRefactor | "execute refactor", "apply move map" | Applied file moves with path updates |

`ExecuteRefactor` and `RefactorPlan` detect the running agent (Claude Code, Codex CLI, Gemini CLI) via `tools/detect-agent.ts` to choose delegation vs. direct execution and avoid circular delegation.

## Bundled content

```
skills/repo-architect/
├── SKILL.md                          # principles + workflow routing
├── frameworks.md                     # 8 framework archetypes + selection heuristics
├── output-spec.md                    # required output sections A-G
├── health-checks.md                  # 6 check categories + severity + JSON schema
├── ci-templates.md                   # hook strategies + GitHub Actions templates
├── intake-questions.md               # AskUserQuestion-structured intake bank
├── move-map-spec.md                  # Move Map format + migration order + rollback
├── tools/
│   ├── detect-agent.ts               # detects the running agent
│   └── index-tree.ts                 # working-tree indexer
└── workflows/                        # 4 workflow playbooks (see table above)
```

## Prerequisites

- **`bun`** — `tools/detect-agent.ts` and `tools/index-tree.ts` run via `bun run`.

## License

MIT — see repository LICENSE.
