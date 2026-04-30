# Audit Prompt Template

Use this as the audit checklist or as a prompt for a separate exploration agent when the host supports one. Replace the `{{...}}` placeholders before using it.

---

You are auditing a `{{STACK}}` repo for "harness engineering" readiness — meaning: how well-equipped is this codebase for autonomous AI agents (Pi, Claude Code, Codex) to do end-to-end work without a human babysitting them?

**Repo:** `{{REPO_PATH}}`
**Stack:** `{{STACK}}` (`{{PACKAGE_MANAGER}}`)
**Context:** `{{REPO_CONTEXT}}` (one-liner — what this repo is and lifecycle stage)

# The 8-Artifact Harness Stack — score each one

For EACH artifact below, return:
- **Status:** ✅ Present / ⚠️ Partial / ❌ Missing
- **Evidence:** specific file paths or "not found"
- **Gap:** what's missing or weak (1-2 lines)

## 1. Cold-start brief (`AGENTS.md` or `CLAUDE.md` at root)
Look for AGENTS.md, CLAUDE.md, README.md at root. Read the first ~150 lines of whatever exists. Assess: does it tell an agent *what this is, how to run it, where rules live, how to test, gotchas*? Or is it a marketing README?

Critical failures to flag:
- Missing entirely
- Says "no build/test commands yet" or similar — but project actually has them (stale)
- All marketing prose, no operator-grade content
- Bloated (>500 lines) — agent context burn

## 2. Rules directory
Per-domain coding standards: naming, logging, error handling, DB conventions, auth, security. Look in:
- `rules/`, `.cursor/rules/`, `docs/rules/`, `.claude/rules/`
- `docs/solutions/`, `docs/patterns/`, `docs/guidelines/`
- `CONTRIBUTING.md`, `UBIQUITOUS_LANGUAGE.md`

Are they imperative ("do X, not Y") or descriptive prose? Imperative is far stronger for agents. Flag stale rules (e.g., rules for a stack the project doesn't use anymore).

## 3. Lint config with rich error messages
Find the linter config for this stack (`{{LINT_CONFIG_PATHS}}`). Sample 3-5 custom rules — do their error messages explain *why* and *how to fix*, or are they terse defaults? Stock messages are descriptions; rich messages are remediation prompts.

## 4. Pre-commit hooks
Look for `.husky/`, `lefthook.yml`, `.git/hooks/pre-commit`, `pre-commit-config.yaml`, `package.json#lint-staged`, or `Cargo.toml#[hooks]`. What runs on commit? Formatter? Type check? Tests on changed files?

## 5. Test suite agents can drive
- Test runner present? (`{{TEST_RUNNER}}`)
- One-liner script? (`npm test`, `./scripts/test.sh`, `cargo test`, etc.) — agents should NOT need to compose toolchain incantations
- Test file count vs source file count (rough ratio)
- Are tests integration or unit-heavy?
- E2E setup if applicable (playwright, cypress, XCUITest, etc.)

## 6. PR review agents (persona reviewers)
- `.github/workflows/` — any AI review workflows? CodeRabbit config (`.coderabbit.yaml`)?
- Any GitHub Actions that spawn Pi/Claude/Codex/etc on PRs?
- Branch protection rules (try `gh api repos/{{OWNER}}/{{REPO}}/branches/main/protection 2>&1`)
- Even basic CI matters here — no CI = no automated quality gate

## 7. Repo-scoped skills and prompts (`.agents/skills/`, `.pi/skills/`, `.claude/skills/`, `.codex/`, `.pi/prompts/`)
Skills checked into the repo (not home dir). What's there? Are they stable wrappers over churning infra (deploy, test, seed), or thin shells? Are they symlinks to fragile sibling paths (audit risk: breaks on fresh clone)?

Concentration check: 5-10 deep skills > 50 shallow ones. Flag sprawl.

## 8. Garbage collection cadence
This is hard to detect from code alone. Look for: weekly review docs, `CHANGELOG.md` with structured entries, `docs/` folder with retros/postmortems, `.github/ISSUE_TEMPLATE/` for "agent slop" or similar, `docs/decisions/` ADRs. Note any signal that PR feedback gets converted into rules/lints over time.

# Bonus signals to capture

`{{STACK_SPECIFIC_BONUS_SIGNALS}}` — read the matching `references/stack-{{STACK}}.md` for stack-specific things to look for here.

Also universal:
- Project structure (monorepo? package count?)
- CI setup — what runs on push/PR?
- Pi package/resource metadata (`package.json#pi`, `.pi/settings.json`, `.pi/skills`, `.pi/prompts`)
- Any `.claude/settings.json` with hooks configured?
- `.mcp.json` — what MCP servers are wired?

# Output format

Use markdown headers per artifact. Keep evidence concrete (file paths, line numbers). End with:

## Summary scorecard (X/8 ✅, Y/8 ⚠️, Z/8 ❌)

## Top 3 highest-leverage gaps
For each: what to add, est. effort (hours), and what unlocks once it's there. Order by leverage × low effort.

## Notable strengths
What's already well-set-up that should be preserved.

## Stack-specific observations
Toolchain caveats agents need to know (Swift project file conflicts, Bun gotchas, Python venv conventions, etc.).

Keep total report under ~1500 words. Be ruthless — gaps not present, but no padding.
