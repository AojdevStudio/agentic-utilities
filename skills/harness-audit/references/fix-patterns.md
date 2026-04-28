# Fix Pattern Templates

Templates for `audit+fix` mode. Each template fills in `{{...}}` placeholders from the audit report and target repo context.

**Universal preamble** — apply before every fix:

```
Repo: `{{REPO_PATH}}` ({{STACK}}, package manager: {{PKG_MGR}})

Constraints from the repo operator docs:
- Surgical fixes only — no refactors of unrelated code
- Functional verification required — run the change and show output
- Do not commit or push unless explicitly requested
- Do not post to external systems unless explicitly requested
- No emojis in committed files unless repo already uses them
- kebab-case for new filenames

Read AGENTS.md / CLAUDE.md and CONTRIBUTING.md first. If commits were explicitly requested, match the repo's commit message conventions (check `git log --oneline -20`).
```

---

## `add-cold-start-brief`

Use when: artifact #1 is ❌ Missing or ⚠️ Partial (stale).

```
Write a fresh AGENTS.md (or fix the existing one) at the repo root. Target: 100-200 lines. NOT a marketing README.

Required sections:
1. **What this is** (1 paragraph) — name, purpose, runtime/stack, lifecycle stage
2. **How to run locally** — exact commands, not prose. Include any env vars to set.
3. **How to test** — the one-liner. If the repo doesn't have one, ALSO add `scripts/test.sh` (see `wrap-test-runner` template).
4. **Where rules live** — pointers to `rules/`, `docs/solutions/`, `CONTRIBUTING.md`, etc.
5. **Gotchas** — concrete past failures. 5-10 bullets max. Each bullet = one rule with reasoning.
6. **Source-of-truth hierarchy** — when docs disagree, which wins.

Anti-patterns to avoid:
- Hardcoded counts ("the project has 14 packages") — these go stale silently
- Directory listings ("the src/ folder contains...") — same problem
- Marketing prose ("a beautifully designed app that...")
- Generic advice ("write tests, follow SOLID") — agents don't need this

If the repo has a stale section saying "no commands yet" but commands DO exist, fix that first.

After writing, ask: "is this lean enough to be useful at cold-start?" If >250 lines, cut.

Verify: read it back end-to-end, then have the user agent (you, simulating cold-start) try to run the test command using only the brief. If you can't, the brief failed.
```

---

## `extract-rules-dir`

Use when: artifact #2 is ⚠️ Partial (rules scattered across many files).

```
Find all coding-standards content currently scattered across the repo. Likely sources:
- AGENTS.md / CLAUDE.md "Conventions" section
- CONTRIBUTING.md "Code style" section
- Random .md files in docs/
- Cursor `.mdc` files
- README sections

Consolidate into a `rules/` directory at repo root, ONE file per domain:
- `rules/naming-conventions.md`
- `rules/error-handling.md`
- `rules/logging.md`
- `rules/{domain-specific}.md` (e.g., `rules/db-conventions.md` for backend, `rules/swiftui-style.md` for iOS)

Each file:
- Imperative voice ("do X, not Y" — never "we tend to prefer X")
- Reasoning included ("because Z happened in production")
- Code examples for both the wrong and right form

After extracting, leave a 1-line pointer in the original location:
> See `rules/naming-conventions.md` for naming standards.

Update AGENTS.md to point to `rules/` in the "Where rules live" section.

Do NOT duplicate rules across files — pick one home, point everywhere else.
```

---

## `add-rich-lint-messages`

Use when: artifact #3 is ⚠️ Partial.

```
Identify the top 10 most-fired lint rules in this repo. Sources:
- Run `{{LINT_CMD}}` and count violations by rule
- Look in PR review history (gh pr list + comments) for repeat patterns
- Check `docs/solutions/` for documented rule violations

For each, rewrite the error message with:
- WHY this rule exists (the consequence in this codebase)
- HOW to fix (specific suggestion, not generic advice)
- WHERE to read more (pointer to rules/ or docs/)

Example transformation:
- BEFORE (stock): `'X' is not allowed`
- AFTER: `Use Y instead — X bypasses our auth middleware (see rules/auth.md). Replace with: Y(...)`

Tools by stack:
- Biome / ESLint: custom rule messages or `noRestrictedSyntax` with `message`
- SwiftLint: `custom_rules` with `message:` field
- ruff: harder — most messages are stock; use `select` + per-file overrides instead
- clippy: same — limited custom message support

Verify: run the linter against a known-bad file, capture output, confirm the new message is clear and actionable.
```

---

## `setup-pre-commit`

Use when: artifact #4 is ❌ Missing.

```
Set up a pre-commit gate appropriate for this stack. Read `references/stack-{{STACK}}.md` from the harness-audit skill for the specific tooling pattern.

Universal requirements:
- Format/lint runs on staged files only (not the whole repo — too slow)
- Type check or compile check runs (project-wide is OK if <10s)
- If type check >10s, demote to pre-push
- Hook FAILS the commit on real violations (not just warnings)
- Auto-fixable issues (formatting) get applied + restaged automatically

Verification steps (REQUIRED):
1. Stage a file with an intentional lint violation. Run the hook command directly (`pre-commit run`, `npx lint-staged`, `.git/hooks/pre-commit`, etc.). Show that it blocks or fixes.
2. Stage a file with a type error. Run the hook command directly. Show that it blocks.
3. Revert the test changes.
4. Show the actual hook output for both cases.

Only run a real `git commit` when the user explicitly requested commit-based verification.

Don't claim done until you've shown evidence.

For Node/TS: bun add -D husky lint-staged, husky init, configure lint-staged in package.json.
For Swift: native `.git/hooks/pre-commit` shell script with swiftlint.
For Python: `pre-commit install` after writing `.pre-commit-config.yaml`.
For Rust: native `.git/hooks/pre-commit` running `cargo fmt --check` and `cargo clippy`.

If commits were explicitly requested, use commit message: `chore: add pre-commit hook running {tools}`. Match repo conventions.
```

---

## `wrap-test-runner`

Use when: artifact #5 is ⚠️ Partial (tests exist but no one-liner).

```
Write `scripts/test.sh` (or equivalent for the stack — Makefile target, package.json script) that:
- Takes optional argument for subset selection (e.g., `./scripts/test.sh ios` vs `./scripts/test.sh watch`)
- Uses `set -euo pipefail` (bash) or strict mode equivalent
- Prints what it's running before running it
- Falls back gracefully if optional pretty-printers (xcbeautify, jq, etc.) aren't installed
- Exits non-zero on failure

For Swift specifically (most common gap):
- Use `OS=latest` not pinned simulator versions — pinned breaks on Xcode mismatch
- Wrap with xcbeautify if available, raw xcodebuild if not
- One subcommand per scheme

Update AGENTS.md "How to test" section to point at the new script.

Verify: actually run the script end-to-end. Show passing test count.

If commits were explicitly requested, use commit message: `chore: add scripts/test.sh wrapping {tool} test runner`.
```

---

## `add-pr-review`

Use when: artifact #6 is ❌ Missing.

```
Add automated PR review. Two paths — pick based on repo:

**Path A: CodeRabbit (recommended for free for public, paid for private)**
- Create `.coderabbit.yaml` at repo root
- Configure language, instructions, file filters
- Add a "reviews" section pointing CodeRabbit at the repo's rules dir
- No GitHub Actions changes needed — CodeRabbit's GitHub App handles it

Example minimal config:
```yaml
language: en-US
reviews:
  profile: chill
  instructions:
    - path: "**/*"
      instructions: "Follow rules in rules/ and docs/solutions/critical-patterns.md"
  path_filters:
    - "!**/dist/**"
    - "!**/node_modules/**"
```

**Path B: Custom Pi/Claude/Codex GitHub Action**
- Create `.github/workflows/ai-review.yml`
- On `pull_request`, spawn Pi, Claude, or Codex with persona prompt
- Posts comment via `gh pr comment` only when explicitly approved and credentials are available

Path A is simpler if CodeRabbit is acceptable. Path B is better if Ossie wants persona-specific reviewers (reliability persona, security persona, etc.) or to avoid CodeRabbit cost.

Default: Path A unless audit found Ossie already runs CodeRabbit elsewhere and is dissatisfied.

Verify: open a draft PR with an intentional issue (a TODO, a console.log) and confirm the review fires within 5 min.
```

---

## `seed-repo-skills`

Use when: artifact #7 is ❌ Missing or ⚠️ Partial.

```
Create 3-5 starter repo skills covering the most agent-touched workflows. Prefer `.agents/skills/` for cross-harness Agent Skills, `.pi/skills/` for Pi-only projects, and `.claude/skills/` for Claude-only projects. Per stack:

**TypeScript:**
- `run-tests` (wraps the test runner with common subset patterns)
- `seed-db` (if applicable — wraps the DB seed script)
- `regen-types` (if using prisma/drizzle/sqlacodegen)
- `deploy-preview` (if applicable)

**Swift:**
- `add-swift-file` (wraps `scripts/add-swift-file.rb` if XcodeGen not used)
- `check-design` (audits view code against project design rules)
- `release-build` (archive + export with signing)

**Python:**
- `run-migration` (alembic / django wrapper)
- `seed-test-data` (fixture loader)

Each skill:
- 1 SKILL.md, 30-60 lines
- `allowed-tools` frontmatter scoped to what the skill actually needs
- Imperative steps, not prose
- Bound to specific commands the project actually has

DO NOT seed skills the project doesn't need. 5-10 deeply-maintained > 50 shallow. Sprawl burns model attention.

If commits were explicitly requested, use commit message: `chore: seed repo skills with {N} starter skills for {workflows}`.
```

---

## `setup-gc-cadence`

Use when: artifact #8 is ❌ Missing.

```
Set up a "garbage collection day" ritual converting agent/PR feedback into permanent rules. Two pieces:

1. **Schedule** — If a scheduling tool is available and the user explicitly wants automation, create a recurring routine (Friday 14:00 local) that:
   - Pulls the week's PR review comments
   - Pulls any "agent slop" issues / labels
   - Drafts proposed rule additions or lint changes
   - Opens a draft PR only when credentials and approval are available

   If no scheduler is available, document the manual Friday ritual instead.

2. **Documentation** — Add a `docs/garbage-collection.md` (or section in AGENTS.md) describing:
   - What it does
   - When it runs
   - How to handle the auto-PR

Optional but recommended: add a `.github/ISSUE_TEMPLATE/agent-slop.yaml` with a label `agent-slop` so noisy agent failures can be tagged for the GC pass.

Verify: for automated cadence, trigger a manual run and confirm it produces output. For manual cadence, verify the docs point to the exact commands or checklist.
```

---

## Execution order

When running `audit+fix` mode and multiple gaps need fixing:

**Parallel-safe (different files):**
- `wrap-test-runner` + `add-pr-review` (touches scripts/ and .github/)
- `setup-pre-commit` + `add-cold-start-brief` (touches .husky/ and AGENTS.md)

**Sequential required (overlapping files):**
- `setup-pre-commit` BEFORE `add-rich-lint-messages` (the second needs the first's lint config in place)
- `wrap-test-runner` BEFORE `add-cold-start-brief` (the brief points at the script)
- `extract-rules-dir` BEFORE `add-rich-lint-messages` (lint messages point at rules)

When in doubt, sequential. Wrong parallelization causes conflicts; wrong sequencing only costs time.
