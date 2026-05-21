# Repo Health Checks

6 categories of health checks for validating repository structure integrity. Each check has a severity level that determines whether it blocks merges, warns, or is informational.

---

## Check Categories

### 1. Structural Checks

Validate the physical shape of the repository tree.

| Check | Description | Severity |
|-------|-------------|----------|
| `allowed-top-level` | Only permitted top-level folders exist | FAIL |
| `required-files` | Essential files present (varies by visibility -- see Required Files Matrix) | FAIL |
| `no-junk-root` | No stray files in root (temp, logs, dumps) | WARN |

#### AI-Era Root File Whitelist

These files should NEVER trigger a "junk in root" warning regardless of repo type:

**Standard whitelist (existing):**
README.md, LICENSE, .gitignore, .editorconfig, package.json, tsconfig.json, Makefile, Dockerfile, docker-compose.yml, .prettierrc, .eslintrc, .nvmrc, .node-version, .tool-versions

**AI-era additions:**
CLAUDE.md, CLAUDE.src.md, AGENTS.md, .mcp.json, justfile, Justfile, .cursorrules, .cursorignore

**AI-era directories (not junk):**
.claude/, .codex/, .gemini/, .cursor/, .agents/

| `folder-placement` | Docker assets in docker/, CI in .github/, etc. | WARN |
| `max-depth` | No folder exceeds configured max depth | WARN |

#### Context-Aware Depth Limits

Max recommended depth varies by repo type:

| Repo Type | Max Recommended Depth | Rationale |
|-----------|----------------------|-----------|
| Code project | 4-6 levels | Standard SRP nesting |
| Monorepo | 5-7 levels | Package nesting adds layers |
| Knowledge base | 8-12 levels | Topic/subtopic nesting is natural |
| Ops hub | 6-10 levels | Workspaces + knowledge sections |

**Detection heuristic:** If a `knowledge/` directory exists with 50+ markdown files, classify as knowledge-heavy and use relaxed depth limits.

| `no-empty-dirs` | No empty directories in tracked tree | INFO |

#### Required Files Matrix

Requirements vary by repo visibility and type:

| File | Public | Private | Knowledge Base | Ops Hub |
|------|--------|---------|----------------|---------|
| README.md | REQUIRED | REQUIRED | REQUIRED | REQUIRED |
| LICENSE | REQUIRED | SKIP | SKIP | SKIP |
| .env.example | REQUIRED (if .env exists) | REQUIRED (if .env exists) | SKIP | SKIP |
| .gitignore | REQUIRED | REQUIRED | RECOMMENDED | REQUIRED |
| CONTRIBUTING.md | RECOMMENDED | SKIP | SKIP | SKIP |

**Detection:** Visibility is determined in AuditRepo Step 0. Repo type inferred from framework classification.

### 2. Metadata Checks

Validate metadata consistency for content/document repos.

| Check | Description | Severity |
|-------|-------------|----------|
| `frontmatter-exists` | All content files have frontmatter | FAIL |
| `required-fields` | Frontmatter contains all required fields | FAIL |
| `field-format` | Metadata fields match expected format (dates, enums) | WARN |
| `location-match` | Metadata topic/category matches folder location | WARN |
| `no-orphan-metadata` | No metadata files without corresponding content | INFO |

### 3. Unknown-Bucket Detection

Catch content drift into catch-all directories.

| Check | Description | Severity |
|-------|-------------|----------|
| `misc-overflow` | misc/unknown/unclassified folders exceed threshold | FAIL |
| `uncategorized-ratio` | % of files in catch-all dirs vs total | WARN |
| `remediation-suggested` | Suggest reclassification for misc items | INFO |

**Thresholds:**
- FAIL if >15% of total files are in unknown/misc/unclassified
- WARN if >5% of files within any topic are uncategorized

### 4. Index Checks

Validate index integrity for metadata-driven repos.

| Check | Description | Severity |
|-------|-------------|----------|
| `all-files-indexed` | Every file on disk appears in index | FAIL |
| `no-stale-entries` | Every index entry points to a real file | FAIL |
| `no-duplicate-ids` | No duplicate IDs in index | FAIL |
| `canonical-paths` | Paths in index are canonical (no ../relative) | WARN |
| `index-freshness` | Index was regenerated after last content change | WARN |

### 5. Module Boundary Checks

Detects violations of module-first architecture. Applicable when `src/modules/` exists.

| Check | Description | Severity |
|-------|-------------|----------|
| `deep-import-detection` | Imports matching `@/modules/<name>/` (anything beyond the module root) from files outside that module | FAIL |
| `provider-sdk-leakage` | Imports of any boundary-listed SDK (Clerk, Supabase, Convex, Drizzle, Stripe, Resend, AI SDK, Composio, Sentry, Cloudflare, Vercel) from files outside the owning module's `internal/providers/` | FAIL |
| `exported-internals` | Module `index.ts` exporting more than 7 symbols, or exporting non-orchestrator/non-IO-type symbols | WARN |
| `misc-bucket-drift` | Directories named `utils2/`, `helpers2/`, `services2/`, `lib2/`, or any `*2/` pattern suggesting bucket overflow | WARN |
| `missing-module-contract` | Module `index.ts` missing JSDoc/TSDoc contract header (purpose, API, IO types, side effects, errors) | WARN |
| `module-entrypoint-missing` | Directory under `src/modules/` without an `index.ts` file | FAIL |

#### Detection Commands

```bash
# Deep imports from outside module
rg "from ['\"]@/modules/\w+/" --type ts --glob '!src/modules/**'

# SDK leakage outside owning module (covers all 13 boundary categories)
rg "from ['\"](@clerk/|@supabase/|convex|drizzle-orm|stripe|resend|@ai-sdk/|@composio/|@sentry/|@cloudflare/|@vercel/|wrangler)" --type ts --glob '!src/modules/**/internal/providers/**'
# Also check bare 'ai' import (Vercel AI SDK):
rg "from ['\"]ai['\"]" --type ts --glob '!src/modules/ai/internal/providers/**'

# Export count per module
for mod in src/modules/*/index.ts; do echo "$mod: $(rg '^export ' "$mod" | wc -l) exports"; done

# Misc bucket drift
fd -t d '(utils|helpers|services|lib|common)\d' src/

# Missing contract header
for mod in src/modules/*/index.ts; do head -5 "$mod" | rg -q '^\s*\*\s*Module:' || echo "MISSING CONTRACT: $mod"; done

# Missing entrypoint
for mod in src/modules/*/; do [ -f "$mod/index.ts" ] || echo "MISSING INDEX: $mod"; done
```

### 6. Policy and ADR Checks

Detect missing architecture decision records and policy surfaces that make repo structure enforceable. Applicable to repos with exported APIs, package boundaries, module entrypoints, services consumed by other packages, or agent-edited contracts.

| Check | Description | Severity |
|-------|-------------|----------|
| `api-doc-policy-missing` | No `docs/api-documentation-policy.md`, `rules/api-documentation.md`, or equivalent section defining which exports require docs | WARN |
| `api-doc-adr-missing` | No ADR/decision note explaining the API documentation standard and rollout posture | WARN |
| `api-doc-tooling-missing` | No TypeDoc/DocC/Sphinx/rustdoc/godoc or equivalent command for API docs | WARN |
| `api-doc-report-missing` | No generated baseline report or coverage output for missing public API docs | INFO |
| `strict-doc-gate-too-early` | Strict doc coverage gate is wired while the baseline report still has known debt | WARN |

#### Detection Commands

```bash
# Policy and ADR surfaces
fd 'api.*documentation|documentation.*api' docs rules .github 2>/dev/null
fd '(adr|decision).*api.*documentation|api.*documentation' docs/adr docs/decisions 2>/dev/null

# TypeScript / Bun / Node tooling
rg '"docs:api|typedoc|eslint-plugin-jsdoc|api-documentation-report' package.json typedoc.json docs scripts 2>/dev/null

# Generic public API surface signal
fd 'index\.(ts|tsx|js|mjs|py|go|rs|swift)$' src packages apps 2>/dev/null
```

**Interpretation:** Missing API docs policy is not a FAIL by itself. It becomes high priority when the repo has exported contracts that multiple agents or packages modify. Prefer adding policy, ADR, and a baseline report before forcing doc coverage to pass.

---

## Severity Levels

| Level | Symbol | Action | CI Behavior |
|-------|--------|--------|-------------|
| FAIL | `[F]` | Block merge/push | Exit code 1, annotation |
| WARN | `[W]` | Print warning, allow proceed | Exit code 0, annotation |
| INFO | `[I]` | Informational only | Exit code 0, no annotation |

---

## JSON Report Schema

Health check results are reported in machine-readable JSON:

```json
{
  "timestamp": "2026-02-03T10:00:00Z",
  "repo_path": "/path/to/repo",
  "framework": "feature-based",
  "summary": {
    "total_checks": 18,
    "pass": 14,
    "fail": 2,
    "warn": 1,
    "info": 1,
    "score": "14/18"
  },
  "checks": [
    {
      "category": "structural",
      "name": "allowed-top-level",
      "severity": "FAIL",
      "status": "PASS",
      "message": "All top-level folders are in allowed list",
      "details": {
        "allowed": ["src", "tests", "docs", "scripts"],
        "found": ["src", "tests", "scripts"],
        "unexpected": []
      }
    },
    {
      "category": "structural",
      "name": "required-files",
      "severity": "FAIL",
      "status": "FAIL",
      "message": "Missing required files: .env.example",
      "details": {
        "required": ["README.md", ".gitignore", ".env.example"],
        "missing": [".env.example"]
      },
      "fix": "Create .env.example with required environment variables"
    }
  ]
}
```

---

## Applying Checks per Framework

Not all checks apply to every framework. Use this matrix:

| Check Category | Topic | Runtime | Feature | Layered | Package | Pipeline | Index | Ops Hub |
|---------------|-------|---------|---------|---------|---------|----------|-------|---------|
| Structural | YES | YES | YES | YES | YES | YES | YES | YES |
| Metadata | YES | NO | NO | NO | NO | OPTIONAL | YES | OPTIONAL |
| Unknown-Bucket | YES | NO | NO | NO | NO | NO | YES | YES |
| Index | OPTIONAL | NO | NO | NO | NO | OPTIONAL | YES | OPTIONAL |
| Module Boundary | NO | NO | YES | YES | YES | NO | NO | NO |
| Policy/ADR | OPTIONAL | OPTIONAL | YES | YES | YES | OPTIONAL | OPTIONAL | YES |
