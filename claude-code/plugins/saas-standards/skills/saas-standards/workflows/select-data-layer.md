# SelectDataLayer Workflow

**Select the right backend and data layer for a SaaS application.**

---

## Trigger

Activate when:
- User asks "What database/backend should I use?"
- User asks about ORM selection (Drizzle, Prisma, etc.)
- User is starting a new SaaS project and hasn't chosen a backend
- ScaffoldAuth workflow needs data layer decisions
- User mentions Supabase, Neon, Convex, or backend selection

---

## Pre-Execution

Load reference: `data-layer.md`

---

## Interview Questions

Before recommending, gather these inputs using **AskUserQuestion tool** (structured options, not prose):

### Question 1: What are you building?

| Option | Description |
|--------|-------------|
| Standard CRUD SaaS | Users, dashboards, settings, typical business app |
| Real-time collaborative app | Multiplayer, live updates, collaboration is core |
| API-first backend | Headless backend consumed by multiple clients |
| Rapid prototype / hackathon | Speed matters most, explore and iterate |

### Question 2: Backend platform preference?

| Option | Description |
|--------|-------------|
| Supabase | All-in-one: auth + DB + storage + real-time + edge functions |
| Neon | Serverless Postgres, bring your own auth and services |
| Convex | Reactive document DB, TypeScript-native, built-in real-time |
| No preference | Help me decide based on my needs |

**If "No preference":** Walk through the decision matrix in `data-layer.md` using follow-up questions about auth needs, real-time requirements, free tier constraints, and deployment target.

### Question 3: Deployment target?

| Option | Description |
|--------|-------------|
| Vercel | Serverless / edge functions |
| Cloudflare Workers / Pages | Edge-first deployment |
| Traditional server | Railway, Fly.io, VPS, Docker |
| Not decided yet | Open to recommendations |

### Question 4: Auth strategy?

| Option | Description |
|--------|-------------|
| Supabase Auth | Built-in with Supabase, GoTrue-based |
| Clerk | Managed auth service, rich UI components |
| Auth.js / NextAuth | Open source, self-hosted |
| Custom / other | Rolling own or using another provider |

### Question 5: ORM preference? (Postgres backends only)

**Skip this question if Convex was selected — Convex uses its own SDK.**

| Option | Description |
|--------|-------------|
| Drizzle | SQL-like TypeScript API, lightweight (~12 KB), code-first schema |
| Prisma | Schema-first with codegen, higher abstraction, mature ecosystem |
| No preference | Help me decide based on my deployment and needs |

**If "No preference":** Present the comparison table from `data-layer.md` and recommend based on deployment target (edge = lean Drizzle, traditional = either works) and team familiarity.

---

## Decision Logic

### Backend Selection

```
IF real-time collaborative AND prefer TypeScript-native:
  -> Convex

IF need all-in-one (auth + storage + real-time) AND <= 2 projects needed:
  -> Supabase

IF need many projects on free tier OR edge-optimized Postgres:
  -> Neon

IF exhausted Supabase free tier:
  -> Neon (100 free projects) OR Convex (40 free deployments)

IF rapid prototype AND don't want to manage auth separately:
  -> Supabase (fastest to full stack)

IF rapid prototype AND want TypeScript-native everything:
  -> Convex
```

### ORM Selection (Postgres only — does NOT apply to Convex)

```
IF edge deployment OR serverless-heavy OR need Supabase RLS:
  -> Lean Drizzle (12 KB, RLS-native, edge-optimized)

IF team already uses Prisma OR prefers higher abstraction:
  -> Lean Prisma (mature ecosystem, greatly improved in v7)

IF no strong preference AND deploying to edge:
  -> Lean Drizzle

IF no strong preference AND traditional server:
  -> Either works — present both, let user decide
```

**Always present both options with criteria. Never force a choice without justification.**

---

## Output

After completing the interview, deliver:

### 1. Backend Recommendation
- Which backend and why (tied to their specific answers)
- Free tier fit assessment
- What they gain and what they give up

### 2. ORM Recommendation (Postgres only)
- Which ORM and why (tied to deployment target and preferences)
- Note: skip entirely for Convex

### 3. Schema Implementation
- Provide the appropriate schema from `data-layer.md` matching Phase 4 tables:
  - Drizzle schema if Drizzle selected
  - Prisma schema if Prisma selected
  - Convex schema if Convex selected

### 4. Connection Setup
- Provide connection code for selected backend + ORM combination
- Include environment variable requirements

### 5. Migration Commands
- Provide the migration workflow for the selected stack
- Include both dev and production commands

### 6. Multi-Tenancy Pattern
- Provide the appropriate access control pattern:
  - RLS for Supabase/Neon
  - Function-level guards for Convex

---

## Integration with ScaffoldAuth

When `ScaffoldAuth` workflow runs and backend/ORM hasn't been chosen:

1. Pause ScaffoldAuth
2. Run SelectDataLayer interview
3. Use selected stack's schema format for Phase 4 tables
4. Use selected backend's auth pattern for Phase 1-3
5. Resume ScaffoldAuth with data layer decisions locked in

---

## Integration with AuditStateManagement

When `AuditStateManagement` workflow runs:

- If backend is Supabase: recommend Supabase Realtime for real-time state layer
- If backend is Convex: note that real-time is built-in (no separate tool needed)
- If backend is Neon: recommend separate real-time solution (Pusher, Socket.io, SSE)

---

*Reference: `data-layer.md` for full comparison tables, schema examples, and migration strategy.*
