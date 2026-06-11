# Update Guidelines

## Core Principle

Agent instruction files are a **map, not an encyclopedia**. Every line must teach something an agent cannot discover from the codebase itself. When in doubt, point to the source of truth rather than restating it.

## The Three Update Operations

### 1. CUT — Remove Discoverable or Volatile Content

Agents are smart. They read package.json, glob for files, and explore directories. Content that restates what agents can find on their own is noise that will eventually become misleading noise.

**Cut these:**

| Content Type | Why Cut | Agent Alternative |
|-------------|---------|-------------------|
| Package.json script lists | Duplicates; drifts when scripts change | Agent reads package.json directly |
| Hardcoded file/directory counts | Goes stale with every PR | Agent can count: `fd -t f '*.sql' migrations/ \| wc -l` |
| Full directory trees | Drift constantly; mislead when stale | Agent uses glob patterns to explore |
| Tech stack version numbers | Updated in package.json/config | Agent reads lockfile or config |
| Generic best practices | Universal advice adds no signal | Agent already knows these |
| One-off fix descriptions | Won't recur; historical clutter | Belongs in git history |
| Verbose explanations | Wastes precious context tokens | Compress to one-liners |

**Example cut:**

Before (stale-prone):
```markdown
## Development Commands

```bash
bun install          # Install dependencies
bun run dev          # Dev server with Turbopack (localhost:3000)
bun run build        # Production build
bun run lint         # Linter
bun run lint:fix     # Lint + auto-fix
bun run format       # Format with Prettier
bun test             # Vitest
bun run test:e2e     # Playwright e2e tests
```
```

After (lean, discovery-friendly):
```markdown
## Commands

Core: `bun run dev` (Turbopack), `bun test` (Vitest), `bun run build`. See `package.json` scripts for the full list.
```

### 2. POINT — Redirect to Source of Truth

When information lives in a canonical location, point there instead of duplicating it.

**Point patterns:**

```markdown
# Instead of listing all env vars:
Environment: Required vars documented in `.env.example`. See `lib/env.ts` for runtime validation.

# Instead of architecture prose:
Architecture: See `docs/ARCHITECTURE.md` for system design. Key decision log in `.planning/STATE.md`.

# Instead of migration details:
Database: Migrations in `supabase/migrations/` (forward-only — never modify existing files, create new ones).

# Instead of full component inventory:
Components: Barrel exports via `index.ts` per feature directory in `components/`.
```

### 3. ADD — Inject Knowledge Agents Can't Infer

This is the highest-value content. Focus on knowledge that exists only in human heads or past debugging sessions.

**Add these:**

| Content Type | Why Add | Example |
|-------------|---------|---------|
| Convention rationale | Agents follow rules better when they understand why | "Server Actions for all mutations — HIPAA audit trail requires it" |
| Gotchas from real failures | Prevents agents from repeating mistakes | "`supa_audit` fails locally — extension unavailable in Docker. Use remote dev database." |
| Workflow sequences | Non-obvious ordering dependencies | "CSV import → preview → mapping → duplicate detection → commit → 5-min undo" |
| Explicit boundaries | Hard constraints agents must never violate | "RLS enforces tenant isolation — never bypass in app code" |
| "Why not X?" rationale | Prevents agents from "improving" things back to a known-bad state | "CSV import over PMS APIs — v2 feature, not v1" |
| Status & context | Orients the agent in the project timeline | "v1.0 shipped (24 phases). Planning v2. See `.planning/`" |

## Validation Checklist

Before finalizing an update:

- [ ] Every remaining line teaches something an agent can't discover from code
- [ ] No hardcoded counts, version numbers, or volatile file listings
- [ ] Discoverable info is pointed-to, not duplicated
- [ ] Gotchas trace to real failures, not theoretical concerns
- [ ] File references have been verified against actual codebase
- [ ] Total line count is under ~150 for root file
- [ ] A new Claude session reading this would understand *how the project thinks*

## Diff Format

For each change, show:

```markdown
### [CUT|POINT|ADD]: description

**Why:** One sentence explaining the improvement.

```diff
- old content
+ new content
```
```

Label each change as CUT (removing), POINT (redirecting), or ADD (new signal) so the user understands the reasoning pattern.
