---
name: harness-audit
description: Audit a repository for autonomous-agent harness readiness across cold-start docs, rules, lint, hooks, tests, PR review automation, repo skills, and garbage-collection cadence. Use for "harness audit", "agent-ready repo", "harness readiness", "make this repo agent-friendly", "harness gaps", or surgical fixes for top harness gaps.
---

# Harness Audit

Audit a codebase against an 8-artifact framework for how well-equipped it is for autonomous AI agents to do end-to-end implementation work. Optionally apply surgical fixes for the highest-leverage gaps.

Framework provenance: Ryan Lopopolo's "Harness Engineering" talk (OpenAI, AI Engineer London 2025). Core thesis: **code is cheap; scarce resources are human time, attention, and model context window.** Codebases that surface the right context to agents at the right time produce more shippable work per human hour.

## Distinct from neighboring skills

- **Repo architecture** — structural design and repo organization. Use that for "how should this repo be laid out?"
- **Adversarial review** — hunts real bugs in working or near-complete implementations. Use that for "is this code correct?"
- **Cold-start doc improvement** — narrowly improves `AGENTS.md` / `CLAUDE.md`. This skill covers that plus seven more artifacts.
- **Harness audit** — asks: *is this repo set up so agents can ship without a human babysitting every step?*

## The 8-artifact stack

For each artifact, score `✅ Present / ⚠️ Partial / ❌ Missing` with concrete path evidence and a one-line gap.

1. **Cold-start brief** — root `AGENTS.md` / `CLAUDE.md` / operator docs. Tells an agent what this is, how to build, how to test, where rules live, and gotchas. Lean, not marketing.
2. **Rules directory** — per-domain coding standards in `rules/`, `.cursor/rules/`, `docs/rules/`, `docs/solutions/`, etc. Imperative rules with reasoning.
3. **Lint config with rich error messages** — ESLint / Biome / SwiftLint / ruff / clippy with custom rules whose messages explain why and how to fix.
4. **Pre-commit hooks** — Husky / lefthook / pre-commit / native git hooks running formatters, type checks, or targeted tests before push/commit.
5. **Test suite agents can drive** — one-liner to run tests (`npm test`, `./scripts/test.sh`, `cargo test`, etc.). Fast enough and deterministic enough for agents.
6. **PR review automation** — CodeRabbit, AI reviewer workflows, persona reviewers, or at minimum CI that gates PRs.
7. **Repo-scoped skills/prompts** — `.agents/skills/`, `.pi/skills/`, `.claude/skills/`, `.codex/`, `.pi/prompts/`, or package-scoped resources. Prefer 5-10 deep resources over many shallow ones.
8. **Garbage-collection cadence** — documented recurring loop that converts agent/PR feedback into permanent rules, lints, docs, or skills.

## Workflow

### Phase 1 — Capture inputs

Inputs may be supplied after `/skill:harness-audit`.

- **Target repo path**: default to the current working directory when omitted.
- **Mode**: `audit` default, `audit+fix`, or `focus:<artifact-name>`.
- **Stack hint**: auto-detect unless supplied.

Do not edit files in `audit` mode. For `audit+fix`, inspect uncommitted changes first and avoid overwriting user work.

### Phase 2 — Detect stack

Read whichever exist:

- `package.json` plus lockfiles (`bun.lockb`, `pnpm-lock.yaml`, `yarn.lock`, `package-lock.json`) → TypeScript / Node.
- `Package.swift`, `*.xcodeproj`, `*.xcworkspace` → Swift.
- `pyproject.toml`, `requirements.txt`, `setup.py`, `poetry.lock`, `uv.lock`, `Pipfile` → Python.
- `Cargo.toml` → Rust.
- `go.mod` → Go/basic generic support.

Read the matching reference from `references/stack-<stack>.md`. If there is no dedicated stack reference, use `references/audit-prompt.md` plus generic judgment.

### Phase 3 — Run the audit

Use `references/audit-prompt.md` as the checklist. Execute the audit directly with available Pi tools (`read`, `bash`, `rg`/`find` through shell as needed). Keep evidence path-specific.

Return:

- 8 artifact scorecards with evidence.
- Summary scorecard (`X/8 ✅, Y/8 ⚠️, Z/8 ❌`).
- Top 3 highest-leverage gaps with effort estimate and what each unlocks.
- Notable strengths.
- Stack-specific observations.

Keep the report under about 1500 words. Do not pad.

### Phase 4 — Mode-specific follow-up

- **`audit`**: stop after the report. Do not edit.
- **`audit+fix`**: fix the top 3 gaps, or fewer if the safe scope is smaller.
- **`focus:<artifact>`**: skip the full audit and handle only that artifact.

Use `references/fix-patterns.md` for fix templates.

Fix rules:

- Make surgical changes only; no unrelated refactors.
- Verify functionally and show output.
- Do not commit or push unless the user explicitly requested commits.
- Do not post to external systems unless explicitly requested.
- Avoid emojis in committed files unless the repo already uses them.
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
| Other | `references/audit-prompt.md` | Adapt the universal artifacts to the toolchain present |

## Fix templates

Use `references/fix-patterns.md`:

- `add-cold-start-brief`
- `extract-rules-dir`
- `add-rich-lint-messages`
- `setup-pre-commit`
- `wrap-test-runner`
- `add-pr-review`
- `seed-repo-skills`
- `setup-gc-cadence`

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

## Notable strengths
- ...

## Stack-specific observations
- ...
```
