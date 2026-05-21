---
name: repo-architect
description: "Repository organization expert — structure, audit, and refactor via 8 framework archetypes. USE WHEN creating a new project, auditing repo structure, or planning refactors."
---

## Customization

**Before executing, check for user customizations at:**
`.claude/repo-architect.local.md`

If this file exists, load and apply any preferences or overrides found there. These override default behavior. If the file does not exist, proceed with skill defaults.

# RepoArchitect

**Repository organization expert — structure, audit, and refactor via 8 framework archetypes.**

Encodes the Repo Architect blueprint: 8 framework archetypes, structured intake, health checks, CI templates, and move map migration planning. Ensures every repo gets intentional structure, not ad-hoc folder creation.

---

## Agent Detection

RepoArchitect adapts its behavior based on which AI agent is running it. This prevents circular delegation (e.g., Codex trying to delegate to itself) and optimizes execution paths.

**Detection tool:** `tools/detect-agent.ts` — run via `bun run tools/detect-agent.ts`

**Detected agents and their capabilities:**

| Agent | Marker | Can Delegate to Codex | Execution Strategy |
|-------|--------|----------------------|-------------------|
| Claude Code | `CLAUDECODE=1` | Yes | Delegate refactors to Codex CLI |
| Codex CLI | `CODEX_SANDBOX` / `CODEX_HOME` | No (circular) | Execute moves directly |
| Gemini CLI | `GEMINI_CLI_*` vars | Yes | Delegate or execute directly |
| Unknown | No markers | No | Manual plan export or direct execution |

**When to detect:** At the start of ExecuteRefactor and RefactorPlan workflows. NewProject and AuditRepo don't need agent detection (no delegation involved).

**How workflows adapt:**
- **ExecuteRefactor:** If running inside Codex, executes file moves directly instead of spawning `codex exec` (which would be circular). If running inside Claude Code, delegates to Codex as designed.
- **RefactorPlan (Step 8):** Adjusts the execution options presented to the user based on which delegations are available from the current agent.

---

## DEFAULT PRINCIPLES

- Choose the organization framework that matches how the repo will be USED (runtime, data, features, packages), not just how it's built.
- Keep the root directory calm: only essential top-level folders + README + configs.
- Favor deterministic structure and conventions (names, metadata, scripts) over "tribal knowledge".
- Treat API documentation policy and ADRs as repo architecture, not cosmetic docs, when exported APIs cross package, module, service, or agent boundaries.
- Prefer metadata-driven indexing for RAG/data repos.
- Prefer runtime-boundary separation for Dockerized services and workflow/runtime products.
- Include tests + CI that catch structural drift (lint-for-repo-shape).
- If the repo already exists, propose a migration plan (move map + path updates + validation).

---

## TOP PRIORITIES

These override all other defaults when applicable.

### 1) Modules First
- Every capability MUST live in `src/modules/<capability>/`.
- Every module MUST expose one public entrypoint (`index.ts`).
- Target: 3-7 exports max (orchestrators + IO types).

### 2) One-Way Dependencies
- External code MAY import only `@/modules/<capability>`.
- Deep imports into module internals MUST be blocked by lint guardrails.

### 3) Provider Boundary Enforcement
- SDK imports (Clerk, Supabase, Convex, Drizzle, Stripe, Resend, AI SDK, Composio, Sentry, Cloudflare, Vercel) MUST stay in owning module internals at `internal/providers/`.
- Skill outputs MUST include checks for provider leakage.

### 4) Auth & Tenancy
- `orgId` is the canonical tenant identifier — never provider-specific IDs in public types.
- Auth module exports `getAuthSession`, `requireAuth`, `requireOrg`, `requireOnboardedOrg`.
- Office/tenant profiles in `src/modules/offices/` backed by DB, keyed on `orgId`.

### 5) Wrap First, Rewrite Later
- ALL refactors MUST establish boundary (entrypoint + contract) before moving internals.
- No big-bang rewrites. Incremental migration only.

### 6) UI Wiring (Next.js App Router)
- Reads: Server Components call module orchestrators directly.
- Writes: Server Actions (at `src/actions/<capability>/`) call one orchestrator each.
- Charts/data: View Models (VM suffix) shaped by modules, not raw DB rows.
- Maximum 3 hops: UI → action → orchestrator.

### 7) Health Checks
- Module boundary violations: deep imports, SDK leakage, exported internals, misc-bucket drift.
- Policy/ADR gaps: missing API documentation policy, missing decision record for public API documentation expectations, and missing baseline report for exported API doc debt.
- Run as part of AuditRepo workflow.

---

## Module Standard

### Canonical Module Shape

```text
src/modules/<capability>/
  index.ts              # Contract + re-exports (3-7 max)
  orchestrators/        # One function per use case (verbNoun naming)
    createClaim.ts
    verifyClaim.ts
  internal/
    types/              # Internal + public IO types
      public.ts         # Exported via index.ts
    providers/          # Optional — SDK/vendor adapters
      clerk/
      stripe/
    repo.ts             # Data access (ORM calls)
    domain.ts           # Pure business rules
```

### Module Contract Template

Every `index.ts` MUST have this header:

```ts
/**
 * Module: <capability>
 * Purpose: <what this capability owns>
 *
 * Public API:
 * - orchestratorA(input): output
 * - orchestratorB(input): output
 *
 * Exported IO Types:
 * - InputType, OutputType
 *
 * Side Effects:
 * - DB writes, cookies, network calls
 *
 * Error Behavior:
 * - typed result union | thrown errors policy
 */
```

### `src/actions/` Convention

Server Actions live at `src/actions/<capability>/<useCase>.action.ts`. Each action is a thin wrapper calling one module orchestrator. Actions are NOT modules — they're wiring.

### DB & Realtime Module Pattern

When app uses Postgres + optional realtime:

```text
src/modules/
  db/                     # System of record
    index.ts
    internal/
      providers/
        supabase/         # Or neon/
        drizzle/          # ORM
        convex/           # Optional, DB features only
  realtime/               # Optional — incremental updates
    index.ts
    internal/
      providers/
        convex/           # Or pusher, etc.
  activity/               # Canonical event log (Postgres-backed)
    index.ts
    orchestrators/
  analytics/              # Dashboard metrics (Postgres-backed)
    index.ts
    orchestrators/
```

Rule: Dashboard initial state from Postgres modules. Realtime layers provide incremental updates only.

---

## Workflow Routing

Route to the appropriate workflow based on the request.

**When executing a workflow, output this notification directly:**

```
Running the **WorkflowName** workflow in the **RepoArchitect** skill to ACTION...
```

### New Project Setup
- "new project", "start repo", "project setup", "scaffold project" -> `workflows/new-project.md`
  - **Context loaded:** `intake-questions.md`, `frameworks.md`, `output-spec.md`, `ci-templates.md`

### Audit Existing Repo
- "audit repo", "repo health", "check structure" -> `workflows/audit-repo.md`
  - **Context loaded:** `frameworks.md`, `health-checks.md`
  - **Note:** AuditRepo now includes module boundary checks (Category 5) when `src/modules/` exists. See `health-checks.md`.

### Plan a Refactor
- "refactor repo", "restructure", "move map" -> `workflows/refactor-plan.md`
  - **Context loaded:** `move-map-spec.md`, `frameworks.md`, `health-checks.md`

### Execute a Refactor
- "execute refactor", "run migration", "apply move map" -> `workflows/execute-refactor.md`
  - **Context loaded:** `move-map-spec.md`, `tools/detect-agent.ts`
  - **Agent-aware:** Detects current agent to decide delegation vs direct execution. See Agent Detection section.

---

## Context Files

| File | Purpose |
|------|---------|
| `frameworks.md` | 8 framework archetypes + selection heuristics |
| `output-spec.md` | Required output sections A-G + quality criteria |
| `health-checks.md` | 6 check categories (including module boundary) + severity + JSON schema |
| `ci-templates.md` | Hook strategies + GitHub Actions templates |
| `intake-questions.md` | AskUserQuestion-structured intake bank |
| `move-map-spec.md` | Move Map format + migration order + rollback |

## Examples

**Example 1: New project setup**
```
User: "I need to set up a new TypeScript API project"
-> Invokes NewProject workflow
-> Runs intake questions, selects framework, generates A-G output
-> User receives canonical tree, CI config, and implementation steps
```

**Example 2: Audit existing repo**
```
User: "Check the health of my repo"
-> Invokes AuditRepo workflow
-> Scans structure, classifies framework, runs health checks
-> User receives score (e.g., 14/18) and prioritized fix list
```

**Example 3: Plan a restructure**
```
User: "My repo is messy, help me restructure it"
-> Invokes RefactorPlan workflow
-> Generates Move Map with phased migration plan
-> Offers Codex-powered execution or manual plan export
```
