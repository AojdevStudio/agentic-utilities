# Workflow: ScaffoldAuth

**Build complete signup + onboarding + route guards from scratch.**

---

## Trigger

User wants to create auth/signup/onboarding for a new SaaS app.

Examples:
- "Build signup for my app"
- "Create authentication"
- "Set up user registration"
- "Build the auth system"

---

## Pre-Execution Checklist

Before building, confirm these with the user (AskUserQuestion):

1. **Framework**: Next.js / React / Express / other?
2. **Auth provider**: Clerk (SHOULD default) / NextAuth / Supabase / custom / other?
3. **Database**: PostgreSQL (Supabase/Neon) + Drizzle (SHOULD default) / Prisma / Convex / other?
4. **Domain**: Generic SaaS / Healthcare-Dental / Fintech / other?
5. **Email verification**: Strict (before onboarding) / Relaxed (banner after)?
6. **Multi-tenant**: Yes (Clerk Organizations — SHOULD default) / No?
7. **Confirm**: `orgId` as canonical tenant FK in all tables

---

## Execution Steps

### Step 1: Create auth module structure

Create:

```text
src/modules/auth/
  index.ts                          # Contract + re-exports
  orchestrators/
    getAuthSession.ts               # Returns AuthSession | null
    requireAuth.ts                  # Returns AuthSession, throws if unauthenticated
    requireOrg.ts                   # Returns { session, orgId }, throws if no org
    requireOnboardedOrg.ts          # Returns { session, orgId }, throws if not onboarded
  internal/
    types/
      public.ts                     # AuthSession, AuthUser (exported via index.ts)
    providers/
      clerk/                        # Or supabase/ — provider adapter
        adapter.ts
```

### Step 2: Write module contract (`index.ts`)

```typescript
/**
 * Module: auth
 * Purpose: Authentication and session management. Provider-agnostic boundary.
 *
 * Public API:
 * - getAuthSession(): AuthSession | null
 * - requireAuth(): AuthSession (throws if unauthenticated)
 * - requireOrg(): { session: AuthSession; orgId: string } (throws if no org)
 * - requireOnboardedOrg(): { session: AuthSession; orgId: string } (throws if not onboarded)
 *
 * Exported IO Types:
 * - AuthSession, AuthUser
 *
 * Side Effects:
 * - Reads cookies/headers for session
 *
 * Error Behavior:
 * - getAuthSession returns null; require* functions throw or redirect
 */

export { getAuthSession } from "./orchestrators/getAuthSession"
export { requireAuth } from "./orchestrators/requireAuth"
export { requireOrg } from "./orchestrators/requireOrg"
export { requireOnboardedOrg } from "./orchestrators/requireOnboardedOrg"
export type { AuthSession, AuthUser } from "./internal/types/public"
```

### Step 3: Create IO types (`internal/types/public.ts`)

```typescript
export interface AuthSession {
  userId: string
  orgId: string | null
  email: string
  role: string | null
  onboardingCompleted: boolean
}

export interface AuthUser {
  id: string
  email: string
  firstName: string | null
  lastName: string | null
}
```

### Step 4: Create provider adapter (`internal/providers/clerk/adapter.ts`)

```typescript
import { auth, currentUser } from "@clerk/nextjs/server"
import type { AuthSession, AuthUser } from "../../types/public"

export async function getClerkSession(): Promise<AuthSession | null> {
  const { userId, orgId } = await auth()
  if (!userId) return null
  const user = await currentUser()
  return {
    userId,
    orgId: orgId ?? null,
    email: user?.emailAddresses[0]?.emailAddress ?? "",
    role: null, // map from Clerk org membership if needed
    onboardingCompleted: false, // read from DB via offices module
  }
}

export async function getClerkUser(): Promise<AuthUser | null> {
  const user = await currentUser()
  if (!user) return null
  return {
    id: user.id,
    email: user.emailAddresses[0]?.emailAddress ?? "",
    firstName: user.firstName,
    lastName: user.lastName,
  }
}
```

### Step 5: Create orchestrators

Each file in `orchestrators/` is a thin function calling the provider adapter:

```typescript
// orchestrators/getAuthSession.ts
import { getClerkSession } from "../internal/providers/clerk/adapter"
import type { AuthSession } from "../internal/types/public"

export async function getAuthSession(): Promise<AuthSession | null> {
  return getClerkSession()
}
```

```typescript
// orchestrators/requireAuth.ts
import { getAuthSession } from "./getAuthSession"
import { redirect } from "next/navigation"
import type { AuthSession } from "../internal/types/public"

export async function requireAuth(): Promise<AuthSession> {
  const session = await getAuthSession()
  if (!session) redirect("/login")
  return session
}
```

```typescript
// orchestrators/requireOrg.ts
import { requireAuth } from "./requireAuth"
import { redirect } from "next/navigation"

export async function requireOrg() {
  const session = await requireAuth()
  if (!session.orgId) redirect("/select-org")
  return { session, orgId: session.orgId }
}
```

```typescript
// orchestrators/requireOnboardedOrg.ts
import { requireOrg } from "./requireOrg"
import { redirect } from "next/navigation"

export async function requireOnboardedOrg() {
  const { session, orgId } = await requireOrg()
  if (!session.onboardingCompleted) redirect("/onboarding")
  return { session, orgId }
}
```

### Step 5: Wire route guards

Middleware and API routes import from `@/modules/auth`:
- `getAuthSession()` for optional auth checks
- `requireAuth()` for protected routes
- `requireOrg()` for org-scoped routes
- `requireOnboardedOrg()` for fully-onboarded routes

See `route-guards.md` for framework-specific patterns.

### Step 6: Create offices module

Create `src/modules/offices/` for DB-backed org profile:

```text
src/modules/offices/
  index.ts
  orchestrators/
    getOfficeProfile.ts
    ensureOfficeProfile.ts
    setOnboardingComplete.ts
  internal/
    types/
      public.ts                     # OfficeProfile
    repo.ts                         # DB queries, keyed on orgId
```

#### Offices contract (`index.ts`)

```typescript
/**
 * Module: offices
 * Purpose: DB-backed organization profiles and onboarding state.
 *
 * Public API:
 * - getOfficeProfile(orgId): OfficeProfile | null
 * - ensureOfficeProfile(orgId, data): OfficeProfile
 * - setOnboardingComplete(orgId): void
 *
 * Exported IO Types:
 * - OfficeProfile
 *
 * Side Effects:
 * - DB reads/writes (organizations table)
 *
 * Error Behavior:
 * - getOfficeProfile returns null if not found
 */

export { getOfficeProfile } from "./orchestrators/getOfficeProfile"
export { ensureOfficeProfile } from "./orchestrators/ensureOfficeProfile"
export { setOnboardingComplete } from "./orchestrators/setOnboardingComplete"
export type { OfficeProfile } from "./internal/types/public"
```

#### Offices IO type (`internal/types/public.ts`)

```typescript
export interface OfficeProfile {
  orgId: string
  name: string
  phone: string
  addressLine1: string | null
  city: string | null
  state: string | null
  zip: string | null
  teamSize: number
  practiceType: string | null // healthcare only
  onboardingCompleted: boolean
}
```

### Step 7: Database Schema

Create tables using the DB module (`modules/db`):
1. Schema definitions inside `modules/db/internal/providers/drizzle/` (or chosen ORM)
2. Use `orgId: string` as the tenant foreign key in all tables — never provider-specific names
3. Tables: users, profiles, organizations, memberships (same as Phase 4 in SKILL.md)

### Step 8: Signup Page

Create `/signup` (or `/register`):
- Email + password + confirm + ToS
- On submit: create user, create session, redirect to /onboarding
- Auth operations go through `@/modules/auth` orchestrators

### Step 9: Onboarding Wizard

Create `/onboarding` with multi-step form:
- Step 1: Personal (first_name, last_name, role)
- Step 2: Organization (org_name, phone, address, team_size)
- Step 3: Optional preferences
- On completion: `modules/offices/setOnboardingComplete(orgId)` → redirect to /dashboard

### Step 10: E2E Tests

Create Playwright tests:

```text
tests/
  auth/
    signup.spec.ts
    onboarding.spec.ts
    login.spec.ts
    route-guards.spec.ts
```

Minimum test cases (same as before): signup → onboarding → dashboard flow, route guards, error states.

---

## Verification Checklist

After building, verify ALL of these:

- [ ] New user can signup with email/password
- [ ] Signup redirects to /onboarding (NOT dashboard)
- [ ] Onboarding collects: first_name, last_name, role, org_name, org_phone, org_location, team_size
- [ ] Onboarding blocks progression on required fields
- [ ] Completion sets onboarding_completed = true
- [ ] Dashboard accessible only after onboarding
- [ ] Route guards redirect correctly for all states
- [ ] Login checks onboarding status for redirect
- [ ] Database has proper schema (users, profiles, organizations, memberships)
- [ ] E2E tests pass for critical path
- [ ] No null fallbacks needed for required profile fields in app pages
- [ ] Auth imports use `@/modules/auth` — no direct provider SDK imports in routes
- [ ] `orgId` used as tenant FK in all DB tables — no provider-specific column names
