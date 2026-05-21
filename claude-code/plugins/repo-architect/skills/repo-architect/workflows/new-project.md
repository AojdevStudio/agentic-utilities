# NewProject Workflow

**Trigger:** "new project", "start repo", "project setup", "scaffold project"

**Context to load:** `intake-questions.md`, `frameworks.md`, `output-spec.md`, `ci-templates.md`

---

## Workflow Steps

### Step 1: Run Intake Questions

Load `intake-questions.md` and deliver questions via AskUserQuestion tool.

**Delivery order (3 calls):**

1. **Call 1 — App & Repo Context:**
   - Product type (web app, backend, CLI, library)
   - Repo state (new, existing-audit, existing-refactor)
   - Primary users (developers, ops, non-technical) — multiSelect

2. **Call 2 — Tech & Deployment:**
   - Tech stack (TypeScript, Python, Go, Rust)
   - Deployment model (Docker, serverless, platform, bare metal)

3. **Call 3 — Scale & Automation:**
   - Team size + scale (solo, medium, large)
   - Compliance constraints (none, HIPAA, SOC2, PCI) — multiSelect
   - CI strictness (standard, strict, light, none)
   - Repo model (single, monorepo, polyrepo)

**If user says "skip" or wants defaults:** Apply defaults from `intake-questions.md` table.

**If repo state is "existing":** Route to `audit-repo.md` or `refactor-plan.md` instead.

---

### Step 2: Match Framework

Load `frameworks.md` and use the selection heuristics table.

**Matching logic:**
1. Map product type + deployment model to primary framework
2. Consider hybrid patterns if multiple signals conflict
3. Select 1 primary framework + 1-2 alternatives

**Present to user:**

| Framework | Fit | Why |
|-----------|-----|-----|
| [Primary] | HIGH | [reason based on their inputs] |
| [Alt 1] | MEDIUM | [trade-off explanation] |
| [Alt 2] | LOW | [why it's less suitable] |

Use AskUserQuestion to confirm:
```
question: "Which framework should we use for your project?"
header: "Framework"
options:
  - label: "[Primary] (Recommended)"
    description: "[1-line fit explanation]"
  - label: "[Alt 1]"
    description: "[1-line trade-off]"
  - label: "[Alt 2]"
    description: "[1-line trade-off]"
```

---

### Step 3: Generate Output A-G

Load `output-spec.md` and produce all 7 sections.

#### Section A: Repo Type + Framework Choice
- State the repo type identified from intake
- Primary framework with rationale
- Trade-off table comparing alternatives

#### Section B: Canonical Directory Tree
- Generate complete tree for the selected framework
- Customize folder names based on their product (not generic placeholders)
- Mark every item as `[NOW]` or `[LATER]`
- Include key files: README.md, .gitignore, .env.example, etc.

#### Section C: Conventions
- File naming conventions matching their stack (e.g., kebab-case for TS)
- Metadata conventions (if applicable)
- Config conventions with .env.example content

#### Section D: Automation Scripts
- `scripts/scaffold.sh` — creates the directory tree
- `scripts/check-structure.sh` — validates structure (from `ci-templates.md`)
- Stack-specific scripts as needed

#### Section E: Health Checks
- Load `health-checks.md`
- Select applicable check categories for their framework
- Configure thresholds and allowed dirs based on their tree
- Include how to run checks locally

#### Section F: CI/CD Guardrails
- Load `ci-templates.md`
- Generate GitHub Actions workflow YAML customized for their repo
- Configure hook strategy based on their strictness preference
- Include check-structure.sh customized for their allowed dirs

#### Section G: Implementation Steps
- 5-10 ordered steps to go from zero to working repo
- Each step: what to do, what to verify
- Include "Run health checks to validate" as a final step

#### Module-First Requirements (when product type is app/SaaS)

- **Section B:** Canonical tree MUST include `src/modules/` with at least `auth/` and one domain module, both with `index.ts` + `orchestrators/` + `internal/`
- **Section D:** Automation MUST include ESLint `no-restricted-imports` config for module boundaries (see SDK boundary table in `output-spec.md` Section D)
- **Section G:** Implementation steps MUST include "Set up module boundaries and ESLint guardrails before writing business logic" as an early step

---

### Step 4: SaaS Note

**If product type is SaaS or web app with user accounts:**

Output a note:

```
Your project includes user accounts. Consider also activating an auth and
onboarding skill or pattern library for:
- Signup → Onboarding → Dashboard flow
- Route guards preventing unauthorized access
- Database schema for users, profiles, organizations

RepoArchitect handles structure; auth/onboarding patterns are out of scope
for this skill.
```

---

## Output Format

Present the A-G output as a single cohesive document with clear section headers. Use code blocks for directory trees, YAML, and scripts. Use tables for trade-offs and conventions.

The output should be copy-pasteable — the user should be able to implement it without further clarification.
