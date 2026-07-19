---
name: harness-audit
description: Audit a repository for autonomous-agent harness readiness and Symphony-style unattended ticket execution readiness across cold-start docs, rules, API documentation policy/ADRs, lint, hooks, tests, PR automation, repo skills, garbage-collection cadence, workflow contracts, evidence, observability, and smoke-ticket evals. Use for "harness audit", "agent-ready repo", "harness readiness", "make this repo agent-friendly", "API docs policy", "Symphony readiness", "prepare this repo for Symphony", "ticket-level agent automation", or surgical fixes for top harness gaps.
---

# Harness Audit

Audit a codebase against a baseline 8-artifact framework for how well-equipped it is for autonomous AI agents to do end-to-end implementation work. When asked for Symphony readiness, add the unattended ticket-execution overlay: workflow contract, disposable workspace bootstrap, validation/evidence, ticket lifecycle, observability, safety, and smoke-ticket eval. Optionally apply surgical fixes for the highest-leverage gaps.

Framework provenance: Ryan Lopopolo's "Harness Engineering" talk (OpenAI, AI Engineer London 2025). Core thesis: **code is cheap; scarce resources are human time, attention, and model context window.** Codebases that surface the right context to agents at the right time produce more shippable work per human hour.

## Distinct from neighboring skills

- **Repo architecture** — structural design and repo organization. Use that for "how should this repo be laid out?"
- **Adversarial review** — hunts real bugs in working or near-complete implementations. Use that for "is this code correct?"
- **Cold-start doc improvement** — narrowly improves `AGENTS.md` / `CLAUDE.md`. This skill covers that plus seven more artifacts.
- **This skill** — asks: *is this repo set up so agents can ship without a human babysitting every step?*

## Mode selection

Default mode is `audit` and is read-only.

Natural-language mapping:

- "harness audit", "agent-ready repo", "harness readiness", "make this repo agent-friendly" → `audit`
- "audit and fix", "fix harness gaps" → `audit+fix`
- "Symphony readiness", "prepare this repo for Symphony", "ticket-level agent automation", "unattended ticket execution" → `symphony-readiness`
- "fix Symphony readiness", "prepare and fix for Symphony" → `audit+fix:symphony`
- "focus on tests/pre-commit/rules/etc." → `focus:<artifact-name>`

Tie-breaker: default to baseline modes unless the user explicitly says Symphony, ticket-level orchestration, unattended execution, scheduler, tracker workflow, or workflow contract.

Accepted focus names: `cold-start-brief`, `rules-dir`, `api-doc-policy`, `lint-messages`, `pre-commit`, `test-suite`, `pr-review`, `repo-skills`, `gc-cadence`, `workflow-contract`, `workspace-bootstrap`, `validate-loop`, `app-validation`, `evidence-protocol`, `ticket-lifecycle`, `observability`, `safety-policy`, `smoke-ticket-eval`.

Numeric aliases:

| Alias | Focus |
| --- | --- |
| `focus:1` | `cold-start-brief` |
| `focus:2` | `rules-dir` |
| `focus:3` | `lint-messages` |
| `focus:4` | `pre-commit` |
| `focus:5` | `test-suite` |
| `focus:6` | `pr-review` |
| `focus:7` | `repo-skills` |
| `focus:8` | `gc-cadence` |
| `focus:symphony-1` | `workflow-contract` |
| `focus:symphony-2` | `workspace-bootstrap` |
| `focus:symphony-3` | `validate-loop` |
| `focus:symphony-4` | `app-validation` |
| `focus:symphony-5` | `evidence-protocol` |
| `focus:symphony-6` | `ticket-lifecycle` |
| `focus:symphony-7` | `observability` |
| `focus:symphony-8` | `safety-policy` |
| `focus:symphony-9` | `smoke-ticket-eval` |

When the user asks for readiness but also asks you to change files, prefer the matching `audit+fix` mode.

## Baseline: the 8-artifact stack

For each artifact, score `✅ Present / ⚠️ Partial / ❌ Missing` with concrete path evidence and a one-line gap.

1. **Cold-start brief** — root `AGENTS.md` / `CLAUDE.md` / operator docs. Tells an agent what this is, how to build, how to test, where rules live, and gotchas. Lean, not marketing.
2. **Rules directory** — per-domain coding standards in `rules/`, `.cursor/rules/`, `docs/rules/`, `docs/solutions/`, etc. Imperative rules with reasoning. For repos with exported APIs, this includes an API documentation policy or ADR.
3. **Lint config with rich error messages** — ESLint / Biome / SwiftLint / ruff / clippy with custom rules whose messages explain why and how to fix.
4. **Pre-commit hooks** — Husky / lefthook / pre-commit / native git hooks running formatters, type checks, or targeted tests before push/commit.
5. **Test suite agents can drive**: one-liner to run tests (`npm test`, `make test`, `cargo test`, etc.). Fast enough and deterministic enough for agents.
6. **PR review automation** — CodeRabbit, AI reviewer workflows, persona reviewers, or at minimum CI that gates PRs.
7. **Repo-scoped skills/prompts** — `.agents/skills/`, `.pi/skills/`, `.claude/skills/`, `.codex/`, `.pi/prompts/`, or package-scoped resources. Prefer 5-10 deep resources over many shallow ones.
8. **Garbage-collection cadence** — documented recurring loop that converts agent/PR feedback into permanent rules, lints, docs, or skills.

## Cross-cutting API documentation check

When a repo exposes package, library, service, or app APIs that agents edit across package boundaries, inspect API documentation as a first-class harness surface:

- policy: `docs/api-documentation-policy.md`, `rules/api-documentation.md`, or equivalent
- decision record: `docs/adr/*api*documentation*`, `docs/decisions/*api*documentation*`, or equivalent
- tooling: TypeDoc, DocC, Sphinx/pdoc, rustdoc, godoc, or stack-native equivalent
- report/backlog: generated missing-docs report or coverage output that separates existing debt from new regressions
- commands: one-liners such as `docs:api`, `docs:api:check`, `docs:api:report`, or a documented equivalent

Treat a missing API documentation policy as a high-leverage rules/GC gap for repos with meaningful exported surfaces. Do not require full coverage before policy exists; prefer a baseline report plus a strict command that can be enabled once debt is paid down.

## Symphony readiness overlay

Use this overlay when the user mentions Symphony, ticket-level orchestration, unattended agents, or preparing repos for an agent scheduler.

Score the overlay separately using `references/symphony-readiness.md`:

1. **`WORKFLOW.md` contract** — repo-local orchestrator contract with tracker states, live tracker preflight, workspace hooks, agent settings, and ticket lifecycle prompt.
2. **Disposable workspace bootstrap** — fresh clone/workspace can install dependencies, prepare services, and clean up without hidden local state.
3. **One-command validate loop** — stable command for the meaningful quality gate.
4. **Agent-visible app validation** — app/browser/log/test path an agent can drive and inspect.
5. **Evidence protocol** — documented proof format for reproduction, validation, screenshots/videos/logs, and PR/ticket handoff.
6. **Ticket/PR lifecycle skill** — repo-scoped workflow for workpad, branch, PR, review comments, human review, rework, and landing.
7. **Agent-readable observability** — token-efficient CLI/API access to local logs, CI failures, browser console, metrics, or traces.
8. **Safety/secrets/workspace policy** — env-backed secrets, destructive-command policy, workspace isolation, sandbox/approval posture.
9. **Smoke-ticket eval** — runnable or documented tiny ticket proving unattended bootstrap → change → validate → evidence → handoff.

Verdict definitions are canonical in `references/symphony-readiness.md`.

## Workflow

### Phase 1 — Capture inputs

Inputs may be supplied in natural language with or without an explicit mode label.

- **Target repo path**: default to the current working directory when omitted.
- **Mode**: `audit` default, `audit+fix`, `symphony-readiness`, `audit+fix:symphony`, or `focus:<artifact-name>`.
- **Stack hint**: auto-detect unless supplied.

Do not edit files in `audit` mode. For `audit+fix`, inspect uncommitted changes first and avoid overwriting user work.

### Phase 2 — Detect stack

Read whichever exist:

- `package.json` plus lockfiles (`bun.lockb`, `pnpm-lock.yaml`, `yarn.lock`, `package-lock.json`) → TypeScript / Node.
- `Package.swift`, `*.xcodeproj`, `*.xcworkspace` → Swift.
- `pyproject.toml`, `requirements.txt`, `setup.py`, `poetry.lock`, `uv.lock`, `Pipfile` → Python.
- `Cargo.toml` → Rust.
- `go.mod` → Go.

If multiple stack markers exist, audit every major stack that has runtime/build impact. For example, a TypeScript frontend plus Python backend should use both stack references and call out cross-stack bootstrap/validation gaps.

Read the matching reference from `references/stack-<stack>.md`. If there is no dedicated stack reference, use `references/audit-prompt.md` plus generic judgment. For `symphony-readiness` or `audit+fix:symphony`, also read `references/symphony-readiness.md`, `references/evidence-protocol.md`, and `references/smoke-ticket-eval.md`.

### Phase 3 — Run the audit

Use `references/audit-prompt.md` as the checklist. Execute the audit directly with available tools (`read`, `bash`, `rg`/`find` through shell as needed). Keep evidence path-specific.

Return:

- 8 baseline artifact scorecards with evidence.
- Summary scorecard (`X/8 ✅, Y/8 ⚠️, Z/8 ❌`).
- Top 3 highest-leverage gaps with effort estimate and what each unlocks.
- Notable strengths.
- Stack-specific observations.

For Symphony modes, additionally return:

- 9-item Symphony readiness overlay scorecard.
- Symphony readiness verdict: `Ready`, `Close`, or `Not ready`.
- Minimum blockers before unattended orchestration, including missing live tracker preflight when a tracker integration is in scope.
- One recommended smoke-ticket eval shape for this repo.

Keep the baseline report under about 1500 words. For Symphony modes, keep the combined report under about 2200 words unless the user asks for depth. Do not pad.

### Phase 4 — Mode-specific follow-up

- **`audit`**: stop after the report. Do not edit.
- **`audit+fix`**: fix the top 3 baseline gaps, or fewer if the safe scope is smaller.
- **`symphony-readiness`**: audit baseline plus Symphony overlay. Do not edit.
- **`audit+fix:symphony`**: fix the smallest set of blockers that moves the repo toward unattended ticket execution. Prefer workflow contract, disposable bootstrap, validation/evidence, and smoke eval over cosmetic docs.
- **`focus:<artifact>`**: read-only by default; skip the full audit and audit only that artifact. For focused edits, use the normal `audit+fix` mode and state the artifact scope in natural language, e.g. `audit+fix, focus pre-commit`.

Use `references/fix-patterns.md` for fix templates. For Symphony workflow seeding, use `references/workflow-template.md`. For proof requirements, use `references/evidence-protocol.md` and `references/smoke-ticket-eval.md`.

Fix rules:

- Make surgical changes only; no unrelated refactors.
- Verify functionally and show output.
- Do not commit or push unless the user explicitly requested commits.
- Do not post to external systems unless explicitly requested.
- Avoid emojis in committed source/config/docs unless the repo already uses them. Status emojis in audit chat reports are allowed.
- Use kebab-case filenames unless the repo convention differs.
- Run fixes sequentially when they touch overlapping files.

### Phase 5 — Report after fixes

Show:

- Updated scorecard (`X/8 → Y/8 ✅`).
- File paths changed.
- Verification evidence.
- Remaining gaps ranked by leverage and effort.

## Stack references

| Stack | Reference | Key tools |
| --- | --- | --- |
| TypeScript / Node | `references/stack-typescript.md` | Biome or ESLint, Husky + lint-staged, vitest / jest, GitHub Actions |
| Swift / iOS | `references/stack-swift.md` | SwiftLint, swift-format, `xcodebuild` wrapper, GitHub Actions macOS runner |
| Python | `references/stack-python.md` | ruff, black, pre-commit, pytest |
| Rust | `references/stack-rust.md` | clippy, rustfmt, cargo test, GitHub Actions |
| Go | `references/stack-go.md` | gofmt, golangci-lint, go test, GitHub Actions |
| Other | `references/audit-prompt.md` | Adapt the universal artifacts to the toolchain present |

Symphony readiness uses cross-stack references: `references/symphony-readiness.md`, `references/evidence-protocol.md`, and `references/smoke-ticket-eval.md`.

## Fix templates

Use `references/fix-patterns.md`.

Focus/fix-template map:

| Focus | Fix template |
| --- | --- |
| `cold-start-brief` | `add-cold-start-brief` |
| `rules-dir` | `extract-rules-dir` |
| `api-doc-policy` | `add-api-doc-policy` |
| `lint-messages` | `add-rich-lint-messages` |
| `pre-commit` | `setup-pre-commit` |
| `test-suite` | `wrap-test-runner` |
| `pr-review` | `add-pr-review` |
| `repo-skills` | `seed-repo-skills` |
| `gc-cadence` | `setup-gc-cadence` |
| `workflow-contract` | `add-workflow-contract` |
| `workspace-bootstrap` | `add-disposable-bootstrap` |
| `validate-loop` | `add-validate-loop` |
| `app-validation` | `add-app-validation` |
| `evidence-protocol` | `add-evidence-protocol` |
| `ticket-lifecycle` | `add-ticket-lifecycle-skill` |
| `observability` | `add-observability-access` |
| `safety-policy` | `add-safety-policy` |
| `smoke-ticket-eval` | `add-smoke-ticket-eval` |

## Output format

```markdown
# Harness Audit — {repo name}

## 1. Cold-start brief
**Status:** ✅ / ⚠️ / ❌
**Evidence:** {file paths or "not found"}
**Gap:** {1-2 lines}

[... artifacts 2-8, same format ...]

## Summary scorecard
X/8 ✅, Y/8 ⚠️, Z/8 ❌

## Top 3 highest-leverage gaps
1. **{name}** — what to add, est. effort, what unlocks
2. ...
3. ...

## Symphony readiness overlay
Include only for `symphony-readiness` and `audit+fix:symphony`.

### 1. WORKFLOW.md contract
**Status:** ✅ / ⚠️ / ❌
**Evidence:** {file paths or "not found"}
**Gap:** {1-2 lines}

[... overlay items 2-9, same format ...]

## Symphony readiness verdict
Ready / Close / Not ready

## Minimum blockers before unattended orchestration
1. ...
2. ...
3. ...

## Recommended smoke-ticket eval shape
- Fixture: ...
- Expected change: ...
- Validation command: ...
- Evidence required: ...
- Cleanup: ...

## Notable strengths
- ...

## Stack-specific observations
- ...
```
