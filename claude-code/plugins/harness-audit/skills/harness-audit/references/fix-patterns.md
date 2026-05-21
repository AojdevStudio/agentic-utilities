# Fix Pattern Templates

Templates for `audit+fix` mode. Each template fills in `{{...}}` placeholders from the audit report and target repo context.

**Universal preamble** — apply before every fix:

```
Repo: `{{REPO_PATH}}` ({{STACK}}, package manager: {{PKG_MGR}})

Constraints from the repo operator docs:
- Surgical fixes only — no refactors of unrelated code
- Functional verification required — run the change and show output
- **Do not commit, push, open PRs, post comments, or call external systems unless explicitly requested.**
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

Verify: read it back end-to-end, then ignore surrounding conversation context and try to run the documented test command using only the brief. If you can't, the brief failed.
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

Preserve surrounding rationale when it is specific and useful. Move only the enforceable rule content; do not erase narrative context that explains why the rule exists unless that rationale moves into the new rule file.

Update AGENTS.md to point to `rules/` in the "Where rules live" section.

Do NOT duplicate rules across files — pick one home, point everywhere else.
```

---

## `add-api-doc-policy`

Use when: `focus:api-doc-policy` is requested, or the audit finds a repo with meaningful exported APIs but no API documentation policy, ADR, tooling, or baseline report.

```markdown
Add the smallest policy/tooling surface that lets agents understand and improve public API documentation without forcing a giant one-shot docstring cleanup.

Universal requirements:
- Add a policy doc such as `docs/api-documentation-policy.md` or `rules/api-documentation.md`.
- Add an ADR or decision note such as `docs/adr/0001-api-documentation-policy.md` when the repo already uses ADRs or decision docs.
- Define what requires docs: exported package APIs, app/server entrypoints consumed by other packages, public types, config schemas, runner/tracker/workspace contracts, and error-prone lifecycle hooks.
- Define what does not require docs: private helpers, obvious local constants, test-only fixtures, and symbols explicitly marked internal.
- Explain the allowed internal marker for the stack, such as `@internal` for TSDoc/JSDoc.
- Add a generated baseline report when a tool can produce one cheaply.
- Do not wire a strict coverage gate into CI until the baseline backlog is low enough to keep CI actionable.

For TypeScript/Bun repos:
1. Add TypeDoc if it is not already present.
2. Add `typedoc.json` using real package/app entrypoints.
3. Add scripts shaped like:
   - `docs:api`
   - `docs:api:check`
   - `docs:api:report`
   - `docs:api:strict`
4. If TypeDoc alone does not give a useful missing-docs backlog, add a small AST-based report script that scans exported top-level declarations and detects leading `/** ... */` comments.
5. Add generated docs output such as `docs/api/` to `.gitignore`.
6. Commit the baseline report, not the generated HTML/API site, unless the repo already tracks generated docs.

For other stacks, adapt to the native documentation checker:
- Python: Sphinx, pdoc, pydocstyle, interrogate, or docstring coverage tooling.
- Swift: DocC plus SwiftLint documentation rules when useful.
- Rust: rustdoc, `cargo doc`, and `#![deny(missing_docs)]` only after baseline debt is handled.
- Go: godoc plus `golint`/staticcheck-style comments where the repo already uses those checks.

Verification:
- Run the report command and ensure it writes the expected baseline report.
- Run the non-strict docs check.
- Run the repo's typecheck/test/verify command if package config changed.
- Run the strict docs command and treat failure as expected only when the report has known missing docs. State the exact missing count instead of claiming the repo is compliant.
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
1. Prefer a scratch worktree or temporary branch so verification does not mutate the user's active index.
2. Stage a file with an intentional lint violation. Run the hook command directly (`pre-commit run`, `npx lint-staged`, `.git/hooks/pre-commit`, etc.). Show that it blocks or fixes.
3. Stage a file with a type error. Run the hook command directly. Show that it blocks.
4. Revert the test changes and restore the original branch/index.
5. Show the actual hook output for both cases.

Only run a real `git commit` when the user explicitly requested commit-based verification.

Don't claim done until you've shown evidence.

For Node/TS: `bun add -D husky lint-staged`, `bunx husky init`, configure lint-staged in package.json.
For Swift: tracked `scripts/git-hooks/pre-commit` plus `make install-hooks`/installer that symlinks into `.git/hooks/`; do not rely on committing `.git/hooks/` directly.
For Python: `pre-commit install` after writing `.pre-commit-config.yaml`.
For Rust: tracked `scripts/git-hooks/pre-commit` plus `make install-hooks`/installer running `cargo fmt --check` and `cargo clippy`; do not rely on committing `.git/hooks/` directly.
For Go: tracked hook installer or pre-commit framework running `gofmt`, `go vet`, and `golangci-lint` when present.

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
      instructions: "Follow rules in rules/ and docs/solutions/"
  path_filters:
    - "!**/dist/**"
    - "!**/node_modules/**"
```

**Path B: Custom AI/agent GitHub Action**
- Create `.github/workflows/ai-review.yml`
- On `pull_request`, spawn an AI agent with persona prompt
- Posts comment via `gh pr comment` only when explicitly approved and credentials are available

Path A is simpler if CodeRabbit is acceptable. Path B is better if you want persona-specific reviewers (reliability persona, security persona, etc.) or to avoid CodeRabbit cost.

Default: Path A unless the audit found CodeRabbit was already tried and rejected.

Verify: validate config syntax and file placement locally. Treat CodeRabbit GitHub App installation as a hard gate: verify it is installed for the repo/org, or print a clear `App not installed — config is dormant` warning and leave PR review automation as partial. Only open a draft PR with an intentional issue when the user explicitly authorizes external GitHub-side verification; otherwise document the exact manual verification command/steps.
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

1. **Schedule** — If a scheduling tool is available and the user explicitly wants automation, ask for or use the repo's documented cadence. If unspecified, document a manual weekly cadence instead of hardcoding a time. The routine should:
   - Pull the week's PR review comments
   - Pull any "agent slop" issues / labels
   - Draft proposed rule additions or lint changes
   - Open a draft PR only when credentials and approval are available

   If no scheduler is available, document the manual weekly ritual instead.

2. **Documentation** — Add a `docs/garbage-collection.md` (or section in AGENTS.md) describing:
   - What it does
   - When it runs
   - How to handle the auto-PR

Optional but recommended: add a `.github/ISSUE_TEMPLATE/agent-slop.yaml` with a label `agent-slop` so noisy agent failures can be tagged for the GC pass.

Verify: for automated cadence, trigger a manual run and confirm it produces output. For manual cadence, verify the docs point to the exact commands or checklist.
```

---

## `add-workflow-contract`

Use when: Symphony overlay #1 is ❌ Missing or ⚠️ Partial.

```
Seed or improve a repo-local `WORKFLOW.md` using `references/workflow-template.md`.

Rules:
- Use obvious placeholders for tracker slug, repo URL, and credentials.
- Do not paste real tokens or personal local paths.
- Match the repo's real validation/bootstrap commands when they exist.
- If commands do not exist yet, point to the scripts this fix also creates or mark the placeholder clearly.
- Include state semantics for Todo, In Progress, Human Review, Rework, Merging, and terminal states, adjusted to the repo's tracker workflow.
- Include explicit handoff criteria and evidence requirements.

Verify: parse the YAML front matter manually or with an available YAML parser; confirm no secrets are present; confirm referenced scripts exist or are marked TODO placeholders.
```

---

## `add-disposable-bootstrap`

Use when: Symphony overlay #2 is ❌ Missing or ⚠️ Partial.

```
Add a minimal disposable bootstrap path such as `scripts/bootstrap.sh` and, when useful, `scripts/verify-ready.sh`.

Requirements:
- strict shell mode (`set -euo pipefail`)
- install dependencies using the repo's package manager
- document required env vars through `.env.example` or AGENTS.md
- avoid machine-specific absolute paths
- avoid destructive cleanup outside the current repo/workspace
- be safe to run in a freshly cloned workspace

Verify in a temporary directory when cheap. If full install is too expensive, run syntax checks and the cheapest dry-run available, then state exactly what was not executed.
```

---

## `add-evidence-protocol`

Use when: Symphony overlay #5 is ❌ Missing or ⚠️ Partial.

```
Add `docs/agent-evidence.md` or an equivalent section using `references/evidence-protocol.md`.

Then wire it into AGENTS.md / repo skills / WORKFLOW.md so agents actually read it.

Must define:
- evidence by change type
- required reproduction signal
- validation command transcript expectations
- screenshot/video/log expectations for UI/runtime work
- where evidence should be attached or stored
- what not to claim without proof

Verify: ensure the cold-start brief or workflow points to the evidence doc.
```

---

## `add-validate-loop`

Use when: Symphony overlay #3 is ❌ Missing or ⚠️ Partial.

```
Add one stable validation command that represents the meaningful pre-handoff quality gate.

Prefer the repo's existing convention:
- `make verify`
- `./scripts/verify.sh`
- `pnpm verify` / `npm run verify`
- `uv run ...`
- `cargo test --workspace && cargo clippy ...`
- `go test ./... && go vet ./...`

The command should run, as applicable:
- lint/format check
- typecheck/compile
- unit/integration tests that are safe locally
- app smoke check when cheap and deterministic

If `wrap-test-runner` already created `scripts/test.sh` or an equivalent test wrapper, extend or call that wrapper from the validate loop instead of creating a parallel, inconsistent test script.

If full validation is slow, expose tiers such as `verify-fast` and `verify-full`, and document which one Symphony agents must run before handoff.

Wire the command into AGENTS.md, WORKFLOW.md, and the ticket lifecycle skill.

Verify: run the fastest real validation tier. If full validation is intentionally skipped due to cost, state the exact skipped command and why.
```

---

## `add-app-validation`

Use when: Symphony overlay #4 is ❌ Missing or ⚠️ Partial.

```
Add the smallest agent-visible app validation path for the repo.

Prefer existing tools and scripts. Do not install a full browser/observability stack unless the repo already uses it.

For web apps, add or document:
- one app launch command (`scripts/launch-app.sh`, `make dev`, `pnpm dev`, etc.)
- one browser validation path (Playwright, Cypress, agent-browser, Chrome DevTools, or existing E2E runner)
- screenshot/video capture command if available
- browser console/server log capture instructions
- shutdown/cleanup command
- deterministic seed/test user instructions when needed

For service/CLI repos, adapt this to a smoke request/CLI invocation plus log capture.

Wire the path into AGENTS.md, the ticket lifecycle skill, or WORKFLOW.md so agents actually use it for UI/runtime changes.

Verify: run the launch command when cheap and show a healthy empty-state/log output. If browser automation is not configured, document the gap explicitly instead of claiming app validation is ready.
```

---

## `add-ticket-lifecycle-skill`

Use when: Symphony overlay #6 is ❌ Missing or ⚠️ Partial.

```
Create a repo-scoped skill/prompt for ticket execution, preferably in the repo's existing skill system. Use `.agents/skills/ticket-lifecycle/SKILL.md` for cross-harness repos when no convention exists.

Include:
- read ticket and acceptance criteria
- maintain one persistent workpad/comment
- branch/worktree policy
- plan/reproduce/implement/validate/handoff loop
- PR creation/update policy
- review comment sweep
- rework policy
- landing policy
- blocker policy

Keep it 60-120 lines. Link out to evidence, testing, and landing docs instead of embedding everything.

Verify: read the skill end-to-end and ensure every linked file exists.
```

---

## `add-observability-access`

Use when: Symphony overlay #7 is ❌ Missing or ⚠️ Partial.

```
Document or add the smallest agent-readable observability path.

Prefer:
- `scripts/logs.sh` for local service logs
- `scripts/ci-failure-summary.sh` for CI logs
- browser console capture instructions for UI apps
- metrics/traces query commands if the repo already has them

Do not invent a full observability stack during a harness fix. Add the thinnest stable wrapper around existing signals.

Verify: run the wrapper against existing logs or show a clear empty-state output.
```

---

## `add-safety-policy`

Use when: Symphony overlay #8 is ❌ Missing or ⚠️ Partial.

```
Add a short `docs/agent-safety.md` or AGENTS.md section covering:
- secrets via env vars only
- required `.env.example` entries
- workspace-only editing expectation
- destructive command policy
- external posting/publishing policy
- sandbox/approval posture
- cleanup boundaries

Verify: run the repo's secret scanner if present, preferably `gitleaks detect --staged` or `gitleaks detect --no-git --source <changed-path>`. If no scanner is present, document that as a remaining safety gap; then grep added files for common token names (`API_KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `PRIVATE_KEY`) and confirm no values were introduced. Do not mark the secret-scan gate as complete unless an actual scanner or CI gate exists.
```

---

## `add-smoke-ticket-eval`

Use when: Symphony overlay #9 is ❌ Missing.

```
Add a local smoke-ticket fixture and runner based on `references/smoke-ticket-eval.md`.

Prefer:
- `docs/agent-evals/smoke-ticket.md`
- `scripts/agent-smoke-eval.sh`

The first version may be a dry-run verifier that checks the fixture, expected marker file, validation command, and evidence section shape. Do not require real tracker credentials unless the user explicitly wants live integration. If the eval is a placeholder, make it fail by default in CI or exclude it from CI with an explicit TODO; do not let a stub silently pass.

Verify: prefer the dry-run path and show pass/fail output. A live smoke-ticket eval may spawn agents, create worktrees, call external trackers, and burn tokens; run it only with explicit user approval. If it is intentionally a template, make that explicit and ensure it exits non-zero until configured.
```

---

## Execution order

When running `audit+fix` mode and multiple gaps need fixing:

**Parallel-safe (different files):**
- `wrap-test-runner` + `add-pr-review` (touches scripts/ and .github/)
- `setup-pre-commit` + `add-cold-start-brief` (touches .husky/ and AGENTS.md)

**Sequential required (overlapping files):**
- `setup-pre-commit` BEFORE `add-rich-lint-messages` (the second needs the first's lint config in place)
- `wrap-test-runner` / `add-validate-loop` BEFORE `add-cold-start-brief` (the brief points at validation scripts)
- `extract-rules-dir` BEFORE `add-rich-lint-messages` (lint messages point at rules)
- `extract-rules-dir` BEFORE `add-api-doc-policy` when the API policy belongs under the rules directory
- `add-api-doc-policy` BEFORE `setup-gc-cadence` when the GC routine should track documentation debt
- `wrap-test-runner` / `add-validate-loop` / `add-disposable-bootstrap` BEFORE `add-workflow-contract` (workflow hooks should reference real commands)
- `add-evidence-protocol` BEFORE `add-ticket-lifecycle-skill` (ticket skill should link to evidence rules)
- `add-app-validation` BEFORE `add-ticket-lifecycle-skill` when ticket skill references UI/runtime proof
- `add-ticket-lifecycle-skill` BEFORE `add-workflow-contract` when the workflow prompt references repo skills
- `add-smoke-ticket-eval` AFTER validation/bootstrap scripts exist

When in doubt, sequential. Wrong parallelization causes conflicts; wrong sequencing only costs time.
