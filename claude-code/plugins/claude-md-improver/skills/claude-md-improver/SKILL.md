---
name: claude-md-improver
description: "USE WHEN: check/audit/update/improve/fix CLAUDE.md or AGENTS.md, CLAUDE.md maintenance, AGENTS.md review, project memory optimization, agent documentation. Audits agent instruction files against modern harness engineering principles (leanness, discoverability, anti-staleness), then applies targeted improvements after approval."
---

# CLAUDE.md / AGENTS.md Improver

Audit, evaluate, and improve agent instruction files (CLAUDE.md, AGENTS.md) using modern harness engineering principles. Also detects fast-moving frameworks (Next.js, Tailwind, React 19, etc.) and recommends training-cutoff-honesty blocks that point agents to local package docs.

**This skill can write to instruction files.** After presenting a quality report and getting user approval, it updates files with targeted improvements.

## Philosophy: Map, Not Encyclopedia

Research from OpenAI's harness engineering team (1M+ lines shipped by agents) and analysis of 2,500+ repositories established clear principles:

- **Context is a scarce resource.** Every token in your instruction file competes with the task, the code, and the relevant docs. A bloated file crowds out the actual work.
- **Too much guidance becomes non-guidance.** When everything is "important," agents pattern-match locally instead of navigating intentionally.
- **Monolithic files rot instantly.** Hardcoded counts, directory listings, and command inventories go stale silently. Agents then confidently look in the wrong place.
- **Agents are smart.** Modern LLMs can read package.json, explore directories, and discover volatile information themselves. Don't document what agents can find, document what they can't infer.

The ideal instruction file is ~100-150 lines serving as a **table of contents** with pointers to deeper sources of truth. It tells agents *how this project thinks*, not *what every file contains*.

## Workflow

### Phase 0: Work-Tracking Context Capture (REQUIRED)

**Before assessment, establish the repository's work-tracking authority.** Agents
need to know where work actually lives so the instruction file can point there
instead of inventing a local task workflow.

Prefer existing repo evidence before asking the user:

- `docs/agents/issue-tracker.md`
- `docs/agents/workflow.md`
- `AGENTS.md` / `CLAUDE.md`
- `git remote -v`
- `gh repo view --json nameWithOwner,url` (if `gh` is available)
- `gh project list --owner <owner> --limit 20` (if the project uses GitHub Projects)
- Tracker config or references for other systems (Linear, Jira, etc.) named in repo docs

Ask the user only when the primary tracker is missing or ambiguous from repo
evidence. Respect whatever the repo declares as authoritative: do not impose a
tracker the project doesn't use. Capture:

1. **Primary tracker:** the system of record for work (e.g. GitHub Issues, Linear, Jira).
2. **Board / project:** owner, number/title, and URL, if applicable.
3. **Mirror / sync:** any one-way sync into a secondary tool for visibility only.

Format for instruction-file injection (when applicable):

```markdown
## Project Tracking

- Primary tracker: <system> (<url>)
- Board / Project: <owner/number-or-title> (<url>)
- Mirror: <secondary tool> receives a one-way sync for visibility only; do not file or update work there unless explicitly asked.
```

Omit any line you can't confirm: empty pointers (`TBD`, `none`, blank
placeholders) are noise. If there is no board, omit the Board line. If there is
no mirror, omit the Mirror line entirely.

**Do not proceed to Phase 1 until the primary tracker is captured or explicitly
declined.**

### Phase 1: Discovery

Find all instruction files in the repository:

```bash
fd -t f '(CLAUDE|AGENTS|\.claude)\.md$' --exclude node_modules | head -50
```

Also check for `.claude.local.md` files.

**File Types & Locations:**

| Type | Location | Purpose |
|------|----------|---------|
| Project root | `./CLAUDE.md` or `./AGENTS.md` | Primary agent context (checked into git, shared with team) |
| Local overrides | `./.claude.local.md` | Personal/local settings (gitignored, not shared) |
| Global defaults | `~/.claude/CLAUDE.md` | User-wide defaults across all projects |
| Subsystem-specific | `./packages/*/CLAUDE.md` | Module-level context in monorepos (keep these minimal) |

Claude auto-discovers instruction files in parent directories.

Also read the root `package.json` to detect fast-moving frameworks. Check `dependencies` and `devDependencies` for: `next`, `tailwindcss`, `react`, `react-router`, `@remix-run/*`, `astro`, `@sveltejs/kit`, `drizzle-orm`, `vite`. Also check for `components.json` or a `components/ui/` directory (shadcn/ui detection). Record which fast-moving frameworks are present, for use in the Phase 3 report and Phase 4 updates. See [references/training-cutoff-honesty.md](references/training-cutoff-honesty.md) for the full detection heuristic.

### Phase 2: Quality Assessment

Evaluate each file against quality criteria. See [references/quality-criteria.md](references/quality-criteria.md) for detailed rubrics.

**Quick Assessment, the Six Questions:**

| Question | Weight | What to check |
|----------|--------|---------------|
| Does it teach how the project *thinks*? | Critical | Architectural intent, conventions, constraints, why-not-just rationale |
| Is volatile data absent? | Critical | No hardcoded counts, no duplicating package.json scripts, no directory listings that will drift |
| Is it lean enough? | High | Under ~150 lines? Every line earning its place? No filler? |
| Does it surface non-obvious gotchas? | High | Concrete past failures, workarounds, edge cases agents would hit |
| Does it point rather than repeat? | Medium | References to docs/, config files, or code rather than restating their contents |
| Is it failure-driven? | Medium | Each item addresses a concrete agent or developer mistake, not theoretical advice |

**Quality Grades:**
- **A (90-100):** Lean, failure-driven, zero staleness risk, teaches intent
- **B (70-89):** Good signal-to-noise, minor bloat or staleness risk
- **C (50-69):** Useful but has hardcoded volatile data or excessive length
- **D (30-49):** Bloated, stale, or mostly generic advice
- **F (0-29):** Missing, severely stale, or actively misleading

### Phase 3: Quality Report Output

**ALWAYS output the quality report BEFORE making any updates.**

```
## Agent Instruction File Quality Report

### Summary
- Files found: X
- Average score: X/100
- Primary concern: [leanness | staleness | missing intent | etc.]

### File-by-File Assessment

#### 1. ./CLAUDE.md (Project Root)
**Score: XX/100 (Grade: X) | ~N lines**

| Criterion | Score | Notes |
|-----------|-------|-------|
| Project intent & conventions | X/25 | ... |
| Anti-staleness | X/25 | ... |
| Leanness | X/20 | ... |
| Non-obvious gotchas | X/15 | ... |
| Discoverability & pointers | X/15 | ... |

**Staleness risks:**
- [Lines that WILL go stale and should be removed or made discoverable]

**Missing signal:**
- [Non-obvious knowledge that would prevent agent failures]

**Recommended cuts:**
- [Content that duplicates discoverable information]

### Training Cutoff Honesty
- Detected fast-moving frameworks: [list or "none"]
- Training cutoff block present in CLAUDE.md/AGENTS.md: [yes/no]
- Recommendation: [add block | already present | n/a]

### Work Tracking
- Primary tracker (Phase 0 capture): [system of record | "user declined"]
- Repo: [owner/name + URL | "unknown"]
- Board / Project: [owner/number/title + URL | "none confirmed"]
- Mirror / sync: [secondary tool one-way sync | "none confirmed" | "not applicable"]
- Project Tracking block present in CLAUDE.md/AGENTS.md: [yes/no]
- Recommendation: [add/update block | already present | n/a (user declined)]
```

### Phase 4: Targeted Updates

After outputting the quality report, ask user for confirmation before updating.

**The Three Update Types:**

1. **Cut:** Remove content that duplicates discoverable info or will go stale
   - Hardcoded file counts ("22 migration files", agents can count)
   - Command lists that mirror package.json (agents read package.json)
   - Directory trees that drift with every PR

2. **Point:** Replace verbose content with pointers to source of truth
   - Instead of listing all env vars: "Required env vars documented in `.env.example`"
   - Instead of architecture prose: "See `docs/ARCHITECTURE.md` for system design"
   - Instead of command inventory: "Run `cat package.json | jq .scripts` for all available commands"

3. **Add:** Inject knowledge agents can't infer from code alone
   - Why a pattern exists (not just that it does)
   - Gotchas from concrete past failures
   - Ordering dependencies or prerequisites
   - Constraints that aren't enforced mechanically
   - **Work-tracking pointers:** From Phase 0 capture. Inject the `## Project Tracking` block so future agents know which tracker and board are the primary work hub.
   - **Mirror / sync note:** Only when confirmed. State that the primary tracker syncs one-way into a secondary tool for visibility, and agents must not file or update work there unless explicitly asked.
   - **Training-cutoff honesty:** For projects using fast-moving frameworks (see `references/training-cutoff-honesty.md`), insert a block acknowledging the model's training cutoff and pointing to local package docs. This prevents the agent from inventing outdated APIs.

**Show diffs for each change with rationale.**

### Phase 5: Apply Updates

After user approval, apply changes using the Edit tool. Preserve existing content structure.

## What Belongs in an Instruction File

**YES, agents can't infer this:**
- Architectural intent and constraints ("RLS enforces tenant isolation; never bypass in app code")
- Non-obvious gotchas from real failures ("audit extension fails locally; use the remote dev database")
- Convention rationale ("Server Actions for all mutations because the audit trail requires it")
- Workflow patterns ("CSV import, preview, mapping, duplicate detection, commit, undo window")
- Boundaries ("migrations are forward-only; never modify existing files")
- Status and context ("v1.0 shipped. Planning v2. See .planning/ for roadmap")
- Training cutoff warnings for fast-moving frameworks (e.g., "This is NOT the Next.js you know. Read node_modules/next/dist/docs/")
- Agent workflow pointers for repos that document them under `docs/agents/`:
  `docs/agents/issue-tracker.md`, `docs/agents/workflow.md`,
  `docs/agents/triage-labels.md`, and `docs/agents/domain.md`.
- Work-hub policy when configured: the declared primary tracker and board are
  the source of truth; do not invent local markdown task workflows when repo
  docs say the tracker owns work tracking.
- Mirror policy when configured: a secondary tool is downstream visibility for
  the primary tracker's sync, not the source of truth or the place agents should
  file or update work by default.

**NO, agents can discover this themselves:**
- Package.json scripts (agents read package.json)
- Exact file counts that change ("22 migration files")
- Full directory trees (agents use glob/find)
- Tech stack versions (agents read config files)
- Generic best practices ("use meaningful variable names")
- One-off fixes unlikely to recur

## For Projects Outgrowing a Single File

**Trigger this recommendation when ANY of these are true:**
- File exceeds ~200 lines
- User mentions maintenance difficulty ("hard to maintain", "keeps going stale", "too long")
- File contains 3+ categories of discoverable content (directory trees, command inventories, env var tables, migration lists, route tables)

Recommend the layered approach from OpenAI's harness engineering:

```
CLAUDE.md              <- lean index (~100 lines): intent, conventions, gotchas, pointers
docs/
├── ARCHITECTURE.md    <- system design, domain map
├── CONVENTIONS.md     <- code style, patterns, naming
├── WORKFLOWS.md       <- development workflows, CI/CD
├── agents/            <- issue tracker, project board, triage, context docs
└── references/        <- external docs reformatted for agents
```

The root file becomes a table of contents. Agents read only the docs they need for the current task.

## Templates

See [references/templates.md](references/templates.md) for instruction file templates by project type.

## Update Guidelines

See [references/update-guidelines.md](references/update-guidelines.md) for detailed guidance on what to add, cut, and point.

## Training Cutoff Honesty

See [references/training-cutoff-honesty.md](references/training-cutoff-honesty.md) for detection heuristics and template blocks.

## User Tips

- **`#` key shortcut**: During a Claude session, press `#` to auto-incorporate learnings into CLAUDE.md
- **Failure-driven updates**: When an agent makes a mistake, add the fix to CLAUDE.md so it never happens again
- **Use `.claude.local.md`**: For personal preferences not shared with team
- **Lean is better**: A 60-line file that teaches intent beats a 300-line file that catalogs the obvious
