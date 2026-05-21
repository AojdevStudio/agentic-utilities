---
name: saas-standards
description: "USE WHEN: audit my SaaS app, set up auth, add route guards, pick a backend, which ORM should I use, review my onboarding flow, state management best practices. Enforces SaaS standards across auth, onboarding, DB, and state."
---

## User Configuration

**Before executing, check for user customizations at:**
`.claude/saas-standards.local.md` in the current project directory.

If this file exists, load and apply any preferences, stack choices, or overrides found there. These override default behavior. If the file does not exist, proceed with skill defaults.

# SaaSStandards

**SaaS Application Development Standards — Signup, Onboarding, Auth, Route Guards**

Enforces non-negotiable standards for any SaaS application build. Prevents the #1 failure mode: building signup flows with only email/password and zero onboarding.

---

## WHEN TO ACTIVATE THIS SKILL

Activate when you see these patterns:

### Direct Requests
- "Build a SaaS app for [domain]"
- "Create user authentication"
- "Build signup flow"
- "Create a web application with user accounts"
- "Build an app with login"

### Context Clues (CRITICAL — activate proactively)
- User describes any app that has users → This skill applies
- User mentions "signup", "login", "auth", "accounts" → This skill applies
- User building Next.js / React / any web framework with auth → This skill applies
- User creating a dashboard → This skill applies (dashboard implies auth)
- User mentions any SaaS domain (dental, medical, legal, fintech) → This skill applies
- User asks about ORM, Drizzle, Prisma, database choice → This skill applies (Phase 4B)
- User mentions Supabase, Neon, Convex, or backend selection → This skill applies (Phase 4B)
- User asks about database schema implementation → This skill applies (Phase 4 + 4B)

### Anti-Pattern Detection
If you find yourself building a signup form with ONLY email + password and NO onboarding step, **STOP IMMEDIATELY**. You are violating SaaS standards. Read this skill.

---

## Workflow Routing

Route to the appropriate workflow based on the request.

**When executing a workflow, output this notification directly:**

```
Running the **WorkflowName** workflow in the **SaaSStandards** skill to ACTION...
```

- Audit an existing app's signup/onboarding flow → `workflows/audit-signup-flow.md`
- Build signup + onboarding from scratch → `workflows/scaffold-auth.md`
- Audit or set up state management architecture → `workflows/audit-state-management.md`
- Choose backend platform and ORM / data layer → `workflows/select-data-layer.md`

---

## Module Integration

SaaS Standards maps to capability modules:

| SaaS Concern | Module | Key Orchestrators |
|-------------|--------|-------------------|
| Authentication | `src/modules/auth/` | `getAuthSession`, `requireAuth`, `requireOrg`, `requireOnboardedOrg` |
| Tenant Profiles | `src/modules/offices/` | `getOfficeProfile`, `ensureOfficeProfile`, `setOnboardingComplete` |
| Domain Features | `src/modules/<domain>/` | Per-project (e.g., `modules/analytics`, `modules/claims`) |

All auth references in this skill mean `@/modules/auth` orchestrators, not `@/lib/auth` or direct provider imports.

> **Provider note:** Clerk is the recommended default auth provider. These patterns work with any provider behind `modules/auth`.

---

## THE NON-NEGOTIABLE STANDARD

### Phase 1: Signup (Minimal — Account Creation Only)

**Route:** `/signup` or `/register`

**Required Fields:**
- email (required, validated, unique)
- password (required, strength-validated)
- confirm_password (required, must match)

**Behaviors:**
- Creates user account in database
- Sends email verification (if applicable)
- Sets `onboarding_completed: false` on user record
- Redirects to `/onboarding` (MANDATORY — never to dashboard)
- Accepts Terms of Service / Privacy Policy checkbox

**Password Standards (NIST 800-63B Rev 4):**
- Minimum 8 characters (with MFA) or 15 characters (without MFA)
- Support >= 64 characters, never truncate
- Allow ALL printable ASCII, spaces, and Unicode
- Check against breached password database (HaveIBeenPwned API) — MANDATORY
- NO composition requirements (no "must include uppercase/symbol")
- NO mandatory password rotation
- Allow paste (support password managers)
- Show strength meter (visual, non-blocking)

**Error Handling:**
- Email already exists → inline error message (or ambiguous for security-sensitive apps)
- Weak/breached password → inline strength indicator + disable submit
- Network error → retry with feedback
- Validation errors → field-level inline messages

**Done when:**
- User account exists in database
- User is authenticated (session/JWT created)
- User is redirected to onboarding (NOT dashboard)

---

### Phase 2: Onboarding (MANDATORY — Blocks Dashboard Access)

**Route:** `/onboarding` (multi-step wizard)

**This phase is NON-NEGOTIABLE. Users CANNOT access the dashboard until onboarding is complete.**

#### Generic SaaS Minimum Fields

| Field | Required | Step | Purpose |
|-------|----------|------|---------|
| first_name | YES | 1 | Basic identity |
| last_name | YES | 1 | Basic identity |
| role / position | YES | 1 | Personalization, permissions |
| company_name / org_name | YES | 2 | Multi-tenancy, workspace |
| company_phone | YES | 2 | Contact, verification |
| company_location | YES | 2 | Timezone, compliance, locale |
| team_size | YES | 2 | Plan selection, feature gating |
| use_case / industry | RECOMMENDED | 3 | Personalization, analytics |
| how_did_you_hear | RECOMMENDED | 3 | Marketing attribution |

#### Healthcare/Dental SaaS Additional Fields

| Field | Required | Step | Purpose |
|-------|----------|------|---------|
| practice_name | YES | 2 | Organization identity |
| practice_type | YES | 2 | Specialty (general, ortho, pedo, endo, etc.) |
| practice_phone | YES | 2 | Contact, appointment routing |
| practice_address | YES | 2 | Location, multi-site support |
| NPI_number | RECOMMENDED | 2 | Provider identification |
| number_of_providers | YES | 2 | Licensing, plan sizing |
| number_of_staff | YES | 2 | Seat-based licensing |
| primary_insurance_types | RECOMMENDED | 3 | Integration setup |
| practice_management_software | RECOMMENDED | 3 | Integration compatibility |
| HIPAA_BAA_accepted | YES | 3 | Compliance requirement |

#### Onboarding Completion Gate

```
User.onboarding_completed === true
  REQUIRES:
    - first_name IS NOT NULL
    - last_name IS NOT NULL
    - role IS NOT NULL
    - organization IS NOT NULL (company_name OR practice_name)
    - organization_phone IS NOT NULL
    - organization_location IS NOT NULL
    - team_size IS NOT NULL
```

**Behaviors:**
- Multi-step wizard (2-4 steps recommended)
- Progress indicator visible
- Back button on each step (except step 1)
- Data saves on each step completion (not just at the end)
- Skip button ONLY on optional fields/steps
- Final step shows summary for confirmation
- On completion: sets `onboarding_completed: true`, redirects to `/dashboard`

---

### Office/Tenant Profile Module

`src/modules/offices/` (or `modules/tenancy/`) owns:
- DB-backed org profile CRUD
- Onboarding completion tracking
- Office metadata (name, phone, address, team size)

Orchestrators:
- `getOfficeProfile(orgId): OfficeProfile | null`
- `ensureOfficeProfile(orgId, data): OfficeProfile` (create-if-missing)
- `setOnboardingComplete(orgId): void`

All keyed on `orgId`, never on provider-specific IDs.

---

### Phase 3: Route Guards (Enforcement Layer)

**Route guards MUST exist to enforce the signup → onboarding → dashboard flow.**

#### Middleware Logic (Pseudocode)

```
function authMiddleware(request):
  session = getAuthSession()  // from @/modules/auth

  if NOT session:
    redirect("/login")
    return

  if NOT session.email_verified AND route != "/verify-email":
    redirect("/verify-email")
    return

  if NOT session.onboarding_completed AND route != "/onboarding":
    redirect("/onboarding")
    return

  // User is authenticated + onboarded → allow access
  next()
```

#### Route Access Matrix

| Route | Unauthenticated | Authenticated (no onboarding) | Fully Onboarded |
|-------|-----------------|-------------------------------|-----------------|
| `/signup` | ALLOW | Redirect to `/onboarding` | Redirect to `/dashboard` |
| `/login` | ALLOW | Redirect to `/onboarding` | Redirect to `/dashboard` |
| `/onboarding` | Redirect to `/login` | ALLOW | Redirect to `/dashboard` |
| `/dashboard` | Redirect to `/login` | Redirect to `/onboarding` | ALLOW |
| `/settings` | Redirect to `/login` | Redirect to `/onboarding` | ALLOW |
| `/api/*` | 401 | 403 | ALLOW |

#### Framework-Specific Patterns

**Next.js (App Router):**
```typescript
// middleware.ts
import { getAuthSession } from '@/modules/auth'

export function middleware(request: NextRequest) {
  const session = await getAuthSession()
  const path = request.nextUrl.pathname

  const publicRoutes = ['/signup', '/login', '/forgot-password']
  const onboardingRoute = '/onboarding'

  if (!session && !publicRoutes.includes(path)) {
    return NextResponse.redirect(new URL('/login', request.url))
  }

  if (session && !session.onboardingCompleted && path !== onboardingRoute) {
    return NextResponse.redirect(new URL('/onboarding', request.url))
  }

  if (session && session.onboardingCompleted && publicRoutes.includes(path)) {
    return NextResponse.redirect(new URL('/dashboard', request.url))
  }
}
```

---

### Phase 4: Database Schema Pattern

> **Module-first note:** Use `orgId` as the canonical tenant key in all tables. Never use provider-specific identifiers as column names. All schema examples below use generic IDs that work with any auth provider.

#### User Table (Authentication Only)

```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  email_verified BOOLEAN DEFAULT FALSE,
  onboarding_completed BOOLEAN DEFAULT FALSE,
  platform_role VARCHAR(50), -- NULL for regular users, 'platform_admin' for app operators
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### Profile Table (Onboarding Data)

```sql
CREATE TABLE profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  first_name VARCHAR(100) NOT NULL,
  last_name VARCHAR(100) NOT NULL,
  role VARCHAR(100) NOT NULL,
  phone VARCHAR(20),
  avatar_url TEXT,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### Organization Table (B2B / Practice)

```sql
CREATE TABLE organizations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(255) NOT NULL,
  phone VARCHAR(20) NOT NULL,
  address_line1 VARCHAR(255),
  city VARCHAR(100),
  state VARCHAR(50),
  zip VARCHAR(20),
  country VARCHAR(100) DEFAULT 'US',
  team_size INTEGER NOT NULL,
  industry VARCHAR(100),
  -- Healthcare specific (nullable for non-healthcare)
  npi_number VARCHAR(20),
  practice_type VARCHAR(100),
  hipaa_baa_accepted BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### Membership Table (User ↔ Organization)

```sql
CREATE TABLE memberships (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
  role VARCHAR(50) NOT NULL DEFAULT 'office_staff', -- office_owner, office_manager, office_staff
  invited_by UUID REFERENCES users(id),
  accepted_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

### Phase 4B: Backend & Data Layer

**This phase determines HOW to implement the Phase 4 schema — which backend platform and which data access approach.**

#### The Three Backend Paradigms

| Backend | Type | ORM Required | Auth Built-in |
|---------|------|-------------|---------------|
| **Supabase** | Hosted Postgres + services | Yes (Drizzle or Prisma) | Yes |
| **Neon** | Serverless Postgres | Yes (Drizzle or Prisma) | Via Neon Auth |
| **Convex** | Reactive document DB | No (own TypeScript SDK) | No (BYO) |

#### ORM Selection (Postgres Backends Only)

Both Drizzle and Prisma are production-ready for TypeScript + Postgres. Selection depends on project needs:

| Factor | Drizzle | Prisma |
|--------|---------|--------|
| **Size** | ~12 KB | ~1.6 MB |
| **Approach** | Code-first, SQL-like | Schema-first + codegen |
| **RLS** | Native support | Limited |
| **Edge/Serverless** | Best-in-class | Improved in v7 |
| **Ecosystem** | Growing, approaching v1.0 | Mature, v7.3 |

**Neither is the default.** Run the `SelectDataLayer` workflow to make an informed choice based on deployment target, team familiarity, and feature needs.

#### Convex Is a Different Paradigm

Convex is NOT Postgres. It uses TypeScript-native schema definitions (`schema.ts`) and query/mutation functions. When using Convex, skip ORM selection entirely — use Convex's own SDK. Real-time reactivity is built-in.

#### Full Reference

- Backend decision tree, free tier comparison, schema examples, migration strategy: `data-layer.md`
- Workflow: `workflows/select-data-layer.md`

#### Data Layer — Postgres Truth, Realtime Optional

**Default stack:** Postgres (Supabase/Neon) + Drizzle ORM.
**Realtime (optional):** Convex for incremental updates.

| Concern | Module | Backed By |
|---------|--------|-----------|
| Schema & migrations | `modules/db` | Postgres + Drizzle |
| Dashboard initial state | `modules/analytics` | Postgres |
| Activity feed (canonical) | `modules/activity` | Postgres |
| Live updates (optional) | `modules/realtime` | Convex |

Rules:
- All canonical data MUST be in Postgres
- Realtime provides incremental overlay only — not the source of truth
- ORM/provider SDKs only inside `modules/db/internal/providers/`
- Convex SDKs only inside `modules/db/internal/providers/convex/` or `modules/realtime/internal/providers/convex/`

---

### Phase 5: Critical Path Testing (E2E)

**These Playwright tests MUST exist for any SaaS app:**

```
✅ Critical Path Tests (MANDATORY)
├── New user can sign up with email/password
├── After signup, user is redirected to /onboarding (NOT dashboard)
├── User cannot access /dashboard before completing onboarding
├── User cannot access /settings before completing onboarding
├── Onboarding collects all required fields (name, role, org, phone, location, team_size)
├── Onboarding blocks progression if required fields are empty
├── After onboarding completion, user is redirected to /dashboard
├── Returning user login redirects to /dashboard (if onboarded)
├── Returning user login redirects to /onboarding (if NOT onboarded)
├── Direct URL to /dashboard while unauthenticated redirects to /login
├── Direct URL to /dashboard while un-onboarded redirects to /onboarding
├── Duplicate email signup shows inline error
├── Weak password shows inline validation
└── App pages assume required fields exist (no null/undefined fallbacks needed)
```

---

### Phase 6: State Management Architecture

**State management is not one problem — it's five separate layers. Each needs the right tool.**

This phase is DIAGNOSTIC, not prescriptive. It does NOT default to any single library.

#### The Five State Layers

| Layer | What It Is | Example Tools |
|-------|-----------|---------------|
| **Server State** | Data from database/API (source of truth is server) | TanStack Query, SWR, Apollo, Server Components |
| **Client State** | UI-only state shared across components | Zustand, Jotai, Redux Toolkit, useState |
| **URL State** | Shareable, bookmarkable state | nuqs, useSearchParams |
| **Form State** | Input management and validation | React Hook Form + Zod, Conform, useActionState |
| **Real-Time State** | Server-pushed live updates | Supabase Realtime, Pusher, Socket.io, SSE |

#### Key Principle: Match Tool to Layer

**Server state (database data) needs server state tools.** TanStack Query gives you query deduplication, cache invalidation, optimistic mutations, and prefetch hydration. Zustand does NOT — it manages client-only state.

**Client state (UI toggles, selections) needs client state tools.** Zustand is lightweight and composable. TanStack Query is NOT for this — it manages server data caching.

**Don't put everything in one tool.** The #1 anti-pattern is using a single global store (Redux/Zustand) for server data, form state, UI toggles, and cached API responses.

#### When to Audit

Run the AuditStateManagement workflow when:
- Building a new SaaS app (get the architecture right from the start)
- App is growing and current patterns feel painful (5+ useState in one component, prop drilling, stale data)
- Adding real-time features, optimistic updates, or multi-user collaboration
- Unsure which library to use or whether you need one at all

**Full reference:** `state-management.md`

---

### Phase 7: UI Wiring

**Server Components for reads:**
- Page components (server) call module orchestrators directly
- Pass typed results (or View Models) to client components as props

**Server Actions for writes:**
- `src/actions/<capability>/<useCase>.action.ts`
- Each action calls exactly one module orchestrator
- Validate input at the action boundary

**View Models:**
- Chart/graph data uses VM suffix: `RevenueTrendVM`, `PatientListVM`
- Shaped for rendering, produced by module orchestrators
- Client components receive VMs, never raw DB rows

---

### Effect Integration (Future)

When service graph complexity warrants it, consider Effect for dependency injection:
- Define service Tags for capabilities with external dependencies
- Create Layer implementations inside module `internal/`
- Wire composition root at `src/runtime/layer.ts`

---

## QUICK REFERENCE

**Key Topics:**
- Signup flow: See "Phase 1: Signup" above
- Onboarding fields: See "Phase 2: Onboarding" above
- Route guards: See "Phase 3: Route Guards" above
- Database schema: See "Phase 4: Database Schema Pattern" above
- Backend & data layer: See "Phase 4B: Backend & Data Layer" above
- E2E tests: See "Phase 5: Critical Path Testing" above
- State management: See "Phase 6: State Management Architecture" above
- Healthcare/dental specifics: See "Healthcare/Dental SaaS Additional Fields" above
- Module integration: See "Module Integration" above
- UI wiring: See "Phase 7: UI Wiring" above
- Office/tenant profiles: See "Office/Tenant Profile Module" above
- DB & realtime strategy: See "Data Layer — Postgres Truth" above

**Reference Documents:**
- Signup & Onboarding deep dive: `signup-onboarding.md`
- Route guard patterns: `route-guards.md`
- Healthcare SaaS specifics: `healthcare-saas.md`
- Backend selection, ORM choice, schema implementation: `data-layer.md`
- State management architecture: `state-management.md`

## Examples

**Example 1: Build a SaaS app**
```
User: "Build a dental practice management SaaS"
-> Invokes ScaffoldAuth workflow
-> Creates signup (email+password), mandatory onboarding wizard, route guards
-> User receives auth system with proper user -> profile -> organization schema
```

**Example 2: Audit existing signup flow**
```
User: "Check if my app's signup flow meets standards"
-> Invokes AuditSignupFlow workflow
-> Checks for onboarding gate, required fields, route guards
-> User receives compliance report with specific violations and fixes
```

**Example 3: Add onboarding to existing app**
```
User: "My app goes straight from signup to dashboard, fix it"
-> Invokes ScaffoldAuth workflow
-> Adds onboarding wizard, route guard middleware, database schema updates
-> User gets mandatory onboarding before dashboard access
```

**Example 4: Audit state management architecture**
```
User: "What state management should I use for my SaaS?"
-> Invokes AuditStateManagement workflow
-> Interviews user about framework, data source, pain points, roadmap
-> Discovers: Next.js App Router + Supabase + planned real-time features
-> Recommends: TanStack Query (server state), Zustand (client UI state only), Supabase Realtime
-> Sets up chosen libraries with proper provider wiring
```

**Example 5: State management setup for new project**
```
User: "Set up state management for my app"
-> Invokes AuditStateManagement workflow
-> Interviews to understand which of the 5 state layers need tooling
-> Does NOT default to any library — recommends based on where state lives
-> Scaffolds chosen tools with query key factories, stores, or providers
```

**Example 6: Backend and ORM selection**
```
User: "What database should I use for my SaaS?"
-> Invokes SelectDataLayer workflow
-> Interviews about app type, real-time needs, deployment target, auth strategy
-> Discovers: edge deployment, exhausted Supabase free tier, need real-time
-> Presents backend comparison (Neon vs Convex) with free tier data
-> User selects Convex → provides Convex schema matching Phase 4 tables
-> No ORM selected (Convex uses own SDK — ORM doesn't apply)
```

**Example 7: ORM selection for existing Supabase project**
```
User: "Should I use Drizzle or Prisma with Supabase?"
-> Invokes SelectDataLayer workflow (ORM-focused path)
-> Interviews about deployment target, team familiarity, RLS needs
-> Presents neutral comparison table with selection criteria
-> User decides based on their constraints → provides matching schema + connection setup
```
