# harness-audit

Audit a repository for autonomous-agent harness readiness across cold-start docs, rules, API documentation policy, lint, hooks, tests, PR automation, repo skills, and garbage-collection cadence. Adds a Symphony-style unattended ticket-execution overlay — workflow contract, disposable bootstrap, validation/evidence, ticket lifecycle, observability, safety, and a smoke-ticket eval — so you can tell whether agents can ship work in this repo without a human babysitting every step.

Framework provenance: Ryan Lopopolo's "Harness Engineering" talk (OpenAI, AI Engineer London 2025). Core thesis: **code is cheap; the scarce resources are human time, attention, and model context window.**

## What it does

The skill runs in one of several modes. Default mode is `audit` and is **read-only**.

| Mode | Trigger | Behavior |
|------|---------|----------|
| `audit` | "harness audit", "agent-ready repo", "harness readiness" | Score the repo against the baseline 8-artifact framework. Read-only. |
| `audit+fix` | "audit and fix", "fix harness gaps" | Audit, then apply surgical fixes for the highest-leverage gaps. |
| `symphony-readiness` | "Symphony readiness", "prepare this repo for Symphony", "unattended ticket execution" | Audit plus the unattended ticket-execution overlay. |
| `audit+fix:symphony` | "fix Symphony readiness" | Symphony-readiness audit plus fixes. |
| `focus:<artifact>` | "focus on tests/pre-commit/rules/etc." | Audit a single artifact (e.g. `focus:test-suite`). |

## Bundled content

```
skills/harness-audit/
├── SKILL.md                          # mode selection + audit framework
└── references/
    ├── audit-prompt.md               # the structured audit prompt
    ├── evidence-protocol.md          # how to capture and present evidence
    ├── fix-patterns.md               # surgical fix library
    ├── symphony-readiness.md         # unattended ticket-execution overlay
    ├── smoke-ticket-eval.md          # end-to-end smoke-ticket evaluation
    ├── workflow-template.md          # workflow-contract template
    ├── stack-typescript.md           # TypeScript stack specifics
    ├── stack-swift.md                # Swift stack specifics
    ├── stack-python.md               # Python stack specifics
    ├── stack-rust.md                 # Rust stack specifics
    └── stack-go.md                   # Go stack specifics
```

11 reference files covering 5 stacks (TypeScript, Swift, Python, Rust, Go) plus the Symphony readiness and fix-pattern libraries.

## License

MIT — see repository LICENSE.
