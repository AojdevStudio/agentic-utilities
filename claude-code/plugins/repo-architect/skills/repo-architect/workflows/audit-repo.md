# AuditRepo Workflow

**Trigger:** "audit repo", "repo health", "check structure"

**Context to load:** `frameworks.md`, `health-checks.md`

---

## Workflow Steps

### Step 0: Detect Repository Visibility

Determine whether the repo is public or private. This affects which health checks apply.

**Detection methods (try in order):**

1. `gh repo view --json isPrivate -q '.isPrivate'` (requires gh CLI + auth)
2. Parse `git remote -v` for github.com — SSH URLs (`git@github.com:`) are often private
3. Heuristic: if no LICENSE file exists, assume private

```
# Primary method
Bash: gh repo view --json isPrivate -q '.isPrivate'
# Returns "true" or "false"

# Fallback
Bash: git remote -v | head -1
# SSH pattern suggests private, HTTPS public (heuristic only)
```

**Store result as:** `repo.visibility: "public" | "private" | "unknown"`
**Pass to:** All downstream checks (Step 3 required-files, Step 4 reports)

---

### Step 1: Scan Repository Structure

Use Glob and Bash tools to capture the current repo state.

**Scan depth:** 2-3 levels from root.

```
# Capture top-level structure
Glob: **/* (depth 1-2)

# Count files per directory
Bash: fd -t f --max-depth 3 | head -100

# Check for key files
Glob: README.md, .gitignore, .env.example, package.json, tsconfig.json, etc.
```

**Capture:**
- Top-level directories (what exists)
- Key configuration files present/missing
- Approximate file count per directory
- Any obvious anomalies (junk in root, deeply nested paths)

---

### Step 1b: Detect AI Tooling Presence

Scan root for AI-era configuration files and directories.

**AI tooling markers:**

| File/Directory | Tool |
|---------------|------|
| `.claude/` | Claude Code |
| `CLAUDE.md` | Claude Code |
| `CLAUDE.src.md` | Claude Code |
| `AGENTS.md` | Claude Code / Codex |
| `.agents/` | Codex |
| `.codex/` | OpenAI Codex |
| `.gemini/` | Google Gemini |
| `.cursor/` | Cursor IDE |
| `.cursorrules` | Cursor IDE |
| `.cursorignore` | Cursor IDE |
| `.mcp.json` | MCP config |
| `justfile` / `Justfile` | Just command runner |

```
# Scan for AI tooling
Glob: .claude/, CLAUDE.md, CLAUDE.src.md, AGENTS.md, .agents/, .codex/, .gemini/, .cursor/, .cursorrules, .cursorignore, .mcp.json, justfile, Justfile
```

**Store result as:** `repo.aiTooling: string[]` (list of detected markers)
**Flag:** If any detected, mark repo as "AI-assisted project"
**Pass to:** Step 3 no-junk-root check (whitelist these files)

---

### Step 2: Classify Against Framework Archetypes

Load `frameworks.md` and match the current structure against all 8 frameworks.

**Classification logic:**
1. Look at top-level folder names → map to framework signals
2. Check for framework-specific markers:
   - `features/` or `modules/` → Feature-Based
   - `domain/`, `application/`, `infrastructure/` → Layered
   - `packages/` + workspace config → Package-Based
   - `ingestion/`, `processing/`, `outputs/` → Data-Pipeline
   - `indexes/`, frontmatter files → Index-First
   - `runtime/`, `build/`, `ops/` → Runtime-Boundary
   - Subject-named folders → Topic-Based
   - Multiple independent top-level workspaces + knowledge/ → Ops Hub
3. Rate fit: HIGH / MEDIUM / LOW / NONE

**Output:**
| Framework | Fit | Evidence |
|-----------|-----|----------|
| Feature-Based | HIGH | Has `src/features/` with co-located components |
| Layered | LOW | No domain/infrastructure separation |
| ... | ... | ... |

**Identify:** Primary match + divergences from that framework's ideal.

---

### Step 3: Run Health Checks

Load `health-checks.md` and run all applicable checks for the classified framework.

#### Structural Checks
- [ ] `allowed-top-level`: List unexpected top-level directories
- [ ] `required-files`: Check for README.md, .gitignore, .env.example (skip LICENSE, .env.example for private repos per visibility matrix)
- [ ] `no-junk-root`: Identify stray files in root (exclude AI tooling files from repo.aiTooling)
- [ ] `folder-placement`: Docker assets in docker/, CI in .github/
- [ ] `max-depth`: Flag paths exceeding context-aware depth limit (4-6 code, 8-12 knowledge, 6-10 ops hub)

#### Metadata Checks (if content/corpus repo)
- [ ] `frontmatter-exists`: Sample content files for frontmatter
- [ ] `required-fields`: Check frontmatter field completeness
- [ ] `location-match`: Verify metadata matches folder context

#### Unknown-Bucket Detection
- [ ] `misc-overflow`: Check for misc/, unknown/, unclassified/ folders
- [ ] `uncategorized-ratio`: Calculate % of files in catch-all dirs

#### Index Checks (if metadata-driven repo)
- [ ] `all-files-indexed`: Compare disk files vs index entries
- [ ] `no-stale-entries`: Verify all index entries point to real files
- [ ] `no-duplicate-ids`: Check for ID collisions

---

### Step 3b: Module Boundary Audit

**Condition:** Only run if `src/modules/` directory exists.

**Actions:**
1. **Deep import scan:** `rg "from ['\"]@/modules/\w+/" --type ts --glob '!src/modules/**'` — flag any results as FAIL
2. **Provider SDK leakage scan:** `rg "from ['\"](@clerk/|@supabase/|convex|drizzle-orm|stripe|resend|@ai-sdk/|@composio/|@sentry/|@cloudflare/|@vercel/|wrangler)" --type ts --glob '!src/modules/**/internal/providers/**'` — flag any results as FAIL (covers all 13 SDK boundary categories)
3. **Export count check:** For each `src/modules/*/index.ts`, count exports. Flag if > 7 as WARN.
4. **Contract header check:** For each `src/modules/*/index.ts`, verify JSDoc contract exists. Flag missing as WARN.
5. **Entrypoint check:** For each directory under `src/modules/`, verify `index.ts` exists. Flag missing as FAIL.
6. **Misc bucket drift:** `fd -t d '(utils|helpers|services|lib|common)\d' src/` — flag any results as WARN.

**Output:** Add "Module Boundary" section to audit report with pass/fail per check.

---

### Step 3c: Policy and ADR Audit

**Condition:** Run for repos with exported package APIs, `src/modules/`, `packages/`, `apps/`, service contracts, SDK/provider boundaries, or agent-edited public entrypoints.

**Actions:**
1. **API documentation policy scan:** Look for `docs/api-documentation-policy.md`, `rules/api-documentation.md`, or an equivalent `CONTRIBUTING.md`/`AGENTS.md` section.
2. **ADR/decision scan:** Look for `docs/adr/*api*documentation*`, `docs/decisions/*api*documentation*`, or an equivalent decision record explaining the standard.
3. **Tooling scan:** For TypeScript, look for `typedoc.json`, `typedoc` in `package.json`, `eslint-plugin-jsdoc`, or scripts named `docs:api`. For other stacks, use DocC, Sphinx/pdoc, rustdoc, godoc, or the stack-native equivalent.
4. **Baseline report scan:** Look for `docs/api-documentation-report.md` or generated coverage output that lists missing public API docs.
5. **Strict gate sanity:** If a strict documentation gate exists, verify the repo also has a baseline report. Flag strict gates that fail large known debt without a staged rollout plan.

**Output:** Add "Policy and ADR" section to the audit report.

**Auto-fix guidance:** When asked to apply fixes, add policy + ADR + non-strict report tooling first. Do not require full JSDoc/TSDoc/docstring coverage in the same refactor unless the user explicitly asks for a documentation wave.

---

### Step 4: Generate Audit Report

Produce two outputs:

#### Machine-Readable JSON Report

Write to `reports/health.json` (or output to screen if no reports/ dir):

```json
{
  "timestamp": "ISO-8601",
  "repo_path": "/path/to/repo",
  "classified_framework": "feature-based",
  "framework_fit": "HIGH",
  "visibility": "public",
  "ai_tooling": ["CLAUDE.md", ".claude/"],
  "summary": {
    "total_checks": 18,
    "pass": 14,
    "fail": 2,
    "warn": 1,
    "info": 1,
    "score": "14/18"
  },
  "checks": [ ... ]
}
```

#### Human-Readable Markdown Report

```
# Repo Audit Report

## Classification
**Detected framework:** Feature-Based (HIGH fit)
**Divergences:** Missing shared/ directory, tests not co-located

## Health Score: 14/18 (78%)

### Failures (Must Fix)
| # | Check | Issue | Fix |
|---|-------|-------|-----|
| 1 | required-files | Missing .env.example | Create with required vars |
| 2 | allowed-top-level | Unexpected `tmp/` dir | Move to .gitignore or remove |

### Warnings (Should Fix)
| # | Check | Issue | Fix |
|---|-------|-------|-----|
| 1 | no-junk-root | notes.txt in root | Move to docs/ |

### Passing (14/18)
All structural, metadata, and index checks passing.

## Policy and ADR
| Check | Status | Evidence | Fix |
|-------|--------|----------|-----|
| api-doc-policy | WARN | not found | Add docs/api-documentation-policy.md or rules/api-documentation.md |
| api-doc-adr | WARN | not found | Add docs/adr/0001-api-documentation-policy.md |

## Recommendations (Priority Order)
1. Create .env.example with required environment variables
2. Remove tmp/ or add to .gitignore
3. Move notes.txt to docs/
4. Consider co-locating tests with source files
```

---

### Step 5: Score and Recommend

**Scoring:** `passing_checks / total_applicable_checks`

**Recommendation tiers:**

| Score | Status | Action |
|-------|--------|--------|
| 90-100% | Healthy | Minor tweaks only |
| 70-89% | Needs attention | Fix FAIL items, address WARN |
| 50-69% | Needs refactor | Consider RefactorPlan workflow |
| <50% | Critical | RefactorPlan strongly recommended |

If score < 70%, suggest:
```
question: "Your repo health score is [X]%. Would you like to create a refactor plan?"
header: "Next step"
options:
  - label: "Create refactor plan (Recommended)"
    description: "Generate a Move Map to restructure the repo"
  - label: "Fix issues manually"
    description: "I'll handle the fixes myself using the recommendations"
  - label: "Skip for now"
    description: "Acknowledge the issues but don't act yet"
```
