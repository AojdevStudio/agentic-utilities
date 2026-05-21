# Output Specification — RepoArchitect

Every recommendation produced by the NewProject workflow MUST include all 7 sections (A-G). This is the quality contract.

---

## Required Output Sections

### Section A: Repo Type + Chosen Framework

**Must include:**
- Identified repo type (web app, backend service, CLI, mobile, library, data/RAG, workflow, infra, monorepo)
- Primary organization framework (from `frameworks.md`)
- 1-2 viable alternatives with trade-off table

**Trade-off table format:**
| Framework | Fit Score | Strengths for This Project | Weaknesses for This Project |
|-----------|-----------|---------------------------|----------------------------|
| Primary | HIGH | ... | ... |
| Alt 1 | MEDIUM | ... | ... |
| Alt 2 | LOW | ... | ... |

---

### Section B: Canonical Directory Tree

**Must include:**
- Complete file tree (not just top-level)
- Every folder and key file named explicitly (no placeholders like `<feature>/`)
- NOW/LATER markers on every item:
  - `[NOW]` — Create immediately during scaffolding
  - `[LATER]` — Create when needed, documented for future

**Format:**
```
repo/                           [NOW]
├── src/                        [NOW]
│   ├── features/               [NOW]
│   │   └── auth/               [NOW]
│   └── shared/                 [NOW]
├── tests/                      [NOW]
│   └── e2e/                    [LATER]
├── docs/                       [LATER]
├── scripts/                    [NOW]
│   └── check-structure.sh      [NOW]
├── .github/workflows/          [LATER]
├── README.md                   [NOW]
├── .env.example                [NOW]
└── .gitignore                  [NOW]
```

#### Module-Aware Tree (when product type is app/SaaS)

The canonical tree MUST include:
```text
src/
  modules/
    auth/
      index.ts
      orchestrators/
      internal/
        providers/
          clerk/
    offices/           # [NOW] if multi-tenant
      index.ts
      orchestrators/
      internal/
    db/                # [NOW] system of record
      index.ts
      internal/
        providers/
          supabase/    # or neon/
          drizzle/
          convex/      # [LATER] optional
    realtime/          # [LATER] optional incremental updates
      index.ts
      internal/
        providers/
          convex/
    activity/          # [LATER] canonical event log (Postgres-backed)
      index.ts
      orchestrators/
    <domain>/          # [NOW] at least one domain module
      index.ts
      orchestrators/
      internal/
  actions/             # [NOW] server action wiring
    <capability>/
  runtime/             # [LATER] Effect composition root
```

---

### Section C: Conventions

**Must include:**

1. **Naming conventions:**
   - File naming (kebab-case, PascalCase, etc.)
   - Folder naming
   - Test file naming (*.test.ts, *.spec.ts, etc.)

2. **Metadata conventions:**
   - Frontmatter requirements (if content repo)
   - Manifest/config file requirements
   - Required fields and validation rules

3. **Configuration conventions:**
   - `.env.example` with all required variables
   - Secrets handling rules (what goes in .env, what goes in vault)
   - Config file placement rules

#### Module Conventions

| Convention | Rule |
|-----------|------|
| Module naming | Lowercase singular capability: `auth`, `billing`, `insurance` |
| IO types | Suffix-free for inputs, descriptive for outputs: `ClaimInput`, `VerificationResult` |
| Orchestrator naming | `verbNoun`: `createClaim`, `verifyEligibility`, `getRevenueTrend` |
| View Models | `VM` suffix: `RevenueTrendVM`, `PatientListVM` |
| Provider adapters | `internal/providers/<vendor>/`: `clerk/`, `stripe/` |

---

### Section D: Automation Scripts

**Must include:**
- `scripts/scaffold.sh` or equivalent — creates the directory tree
- `scripts/check-structure.sh` — validates the tree matches spec
- Domain-specific scripts as needed:
  - If data/RAG: indexing + chunking rules
  - If Docker: Dockerfile + compose placement rules
  - If monorepo: workspace management scripts

#### Module Boundary Lint Config

Scaffold output MUST include an ESLint `no-restricted-imports` config covering all SDK boundary categories. The 13 SDK boundary categories are:

| SDK | npm scope | Owning module (example) |
|-----|-----------|------------------------|
| Clerk (auth) | `@clerk/` | `src/modules/auth/` |
| Supabase | `@supabase/` | `src/modules/db/` |
| Convex | `convex` | `src/modules/db/` or `realtime/` |
| Drizzle ORM | `drizzle-orm` | `src/modules/db/` |
| Stripe | `stripe` | `src/modules/billing/` |
| Resend | `resend` | `src/modules/notifications/` |
| AI SDK (Vercel) | `@ai-sdk/`, `ai` | `src/modules/ai/` |
| Composio | `@composio/` | `src/modules/integrations/` |
| Sentry | `@sentry/` | `src/modules/monitoring/` |
| Cloudflare | `@cloudflare/`, `wrangler` | `src/modules/edge/` |
| Vercel | `@vercel/` | `src/modules/hosting/` |
| PostHog | `posthog-js`, `posthog-node` | `src/modules/analytics/` |
| tRPC | `@trpc/` | `src/modules/api/` |

The config MUST produce per-SDK error messages that name the exact owning module so violations are self-explanatory.

---

### Section E: Tests & Repo Health Checks

**Must include:**
- Health check definitions (from `health-checks.md`)
- Which checks apply to this repo type
- Check configuration (allowed dirs, required files, thresholds)
- How to run checks locally
- Include Category 5 (Module Boundary Checks) from `health-checks.md` when `src/modules/` is present

---

### Section F: CI/CD Guardrails

**Must include:**
- Hook recommendation (pre-push vs pre-commit) with rationale
- GitHub Actions workflow YAML (from `ci-templates.md`)
- Failure messages and suggested fixes
- Artifact configuration
- CI workflow MUST include `eslint --rule 'no-restricted-imports: error'` or equivalent lint step to enforce module boundaries on every PR

---

### Section G: Implementation Steps

**Must include:**
- 5-10 step implementation plan
- Steps ordered for minimal breakage
- Each step specifies: what to do, what to verify, rollback if needed
- If existing repo: migration steps from `move-map-spec.md`

---

## Quality Criteria

| Criterion | Test |
|-----------|------|
| Implementable without guessing | Every file/folder has a specific name, not a placeholder |
| Trade-offs visible | Pros/cons in tables, not buried in prose |
| NOW/LATER separation | Every item in the tree is marked |
| Specific file names | No `<something>/` or `[your-feature]/` patterns |
| Conventions are enforced | Each convention has a corresponding health check |
| CI catches drift | At least one CI job validates structure |
