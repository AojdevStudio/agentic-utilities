# Data Layer Architecture

**Backend Selection, ORM Choice, and Schema Implementation for SaaS Applications**

Reference document for Phase 4B of SaaS Standards. Loaded when data layer decisions are needed.

---

## Backend Selection Decision Tree

### The Three Paradigms

| Backend | Type | Data Model | Query Language | Auth Built-in | Real-time Built-in | Free Projects |
|---------|------|------------|----------------|---------------|-------------------|---------------|
| **Supabase** | Hosted Postgres + Services | Relational (SQL) | SQL / PostgREST API | Yes (GoTrue) | Yes (Channels) | 2 |
| **Neon** | Serverless Postgres | Relational (SQL) | SQL | Via Neon Auth | No | 100 |
| **Convex** | Reactive Document Database | Document (TypeScript) | TypeScript functions | No (BYO) | Yes (native) | 40 deployments |

### When to Choose Each

**Choose Supabase when:**
- You need an all-in-one platform (auth + storage + real-time + edge functions)
- You want Row-Level Security (RLS) for multi-tenancy
- Your team knows SQL / Postgres
- You want a dashboard UI for database management
- You're building a standard CRUD SaaS with real-time features

**Choose Neon when:**
- You need serverless Postgres that scales to zero (cost optimization)
- You want database branching for preview environments / CI
- You're deploying on Vercel/Cloudflare and want edge-optimized Postgres
- You need multiple projects on free tier (100 vs Supabase's 2)
- You're bringing your own auth (Clerk, Auth.js, etc.) and don't need Supabase's bundled services

**Choose Convex when:**
- You need built-in reactivity (data changes push to UI automatically)
- You prefer TypeScript-native schema and queries (no SQL)
- You're building real-time collaborative features as a core requirement
- You want zero-config caching and optimistic updates
- Your data model fits a document pattern better than strict relational

### Free Tier Comparison (Verified January 2026)

| Resource | Supabase Free | Neon Free | Convex Free |
|----------|--------------|-----------|-------------|
| **DB Storage** | 500 MB | 500 MB/project | 512 MB |
| **Bandwidth** | 5 GB + 5 GB cached | 5 GB | 1 GB (DB) + 1 GB (files) |
| **Auth Users** | 50,000 MAU | 60,000 MAU (Neon Auth) | N/A (BYO) |
| **File Storage** | 1 GB | N/A | 1 GB |
| **Real-time** | 200 conns, 2M msgs/mo | N/A | Native (within function calls) |
| **Serverless Functions** | 500K invocations/mo | N/A | 1M calls/mo |
| **Compute** | Shared CPU, 500 MB RAM | 100 CU-hrs/mo | 20 GB-hrs/mo |
| **Max Projects** | 2 | 100 | 40 deployments |
| **Exceed Behavior** | Hard cap, project pauses after 1wk inactivity | Compute suspends until cycle resets | Hard cap (Free) or overage billing (Starter) |
| **Paid Starts At** | $25/mo (Pro) | $5/mo (Launch) | $25/member/mo (Pro) |

### Decision Matrix

| If you need... | Best fit |
|---------------|----------|
| Most features on free tier (auth + storage + real-time + edge) | Supabase |
| Most free projects (prototyping, experimentation) | Neon (100 projects) |
| Lowest entry cost to paid tier | Neon ($5/mo) |
| Built-in auth with no extra setup | Supabase |
| Database branching for CI/preview environments | Neon |
| Native real-time reactivity (data auto-pushes to UI) | Convex |
| TypeScript-first, no SQL | Convex |
| Standard Postgres compatibility (extensions, tools) | Supabase or Neon |
| Edge/serverless optimized with scale-to-zero | Neon or Convex |
| Exhausted Supabase free tier (need more projects) | Neon (100 projects) or Convex (40 deployments) |

---

## ORM Selection for Postgres Backends

**Applies to: Supabase and Neon only.** Convex does not use ORMs — see "Convex Data Patterns" below.

When using a Postgres backend, you need a way to define schemas, run queries with type safety, and manage migrations. The two leading TypeScript ORMs are **Drizzle** and **Prisma**.

### Comparison (As of January 2026)

| Factor | Drizzle (v1.0 RC) | Prisma (v7.3) |
|--------|-------------------|---------------|
| **Architecture** | Pure TypeScript, code-first | Pure TypeScript (Rust engine removed in v7) |
| **Bundle Size** | ~12 KB | ~1.6 MB |
| **Schema Approach** | TypeScript code defines schema | Prisma Schema Language (.prisma) + codegen |
| **Type Updates** | Instant (schema IS the types) | Requires `prisma generate` after changes |
| **SQL Control** | High — API mirrors SQL syntax | Abstracted behind Prisma Client API |
| **Raw SQL** | First-class, type-safe | Supported, improved in v7.3 |
| **Migrations** | Drizzle Kit CLI (generate, push, pull) | Prisma Migrate (diff-based) |
| **RLS Support** | Yes — critical for Supabase multi-tenancy | Limited — requires raw SQL workarounds |
| **Edge/Serverless** | Best-in-class (12 KB, no dependencies) | Greatly improved in v7 (1.6 MB, no Rust binary) |
| **Supabase Integration** | Excellent — official docs, RLS-native | Good — standard PG connection |
| **Neon Integration** | Excellent — first-class driver support | Good — adapter available |
| **Ecosystem Maturity** | Approaching v1.0 (94% complete) | Mature (v7.3, years of production) |
| **Introspection** | `drizzle-kit pull` | `prisma db pull` |
| **Studio/GUI** | Drizzle Studio (browser-based) | Prisma Studio (desktop) |

### Selection Criteria

**Lean toward Drizzle when:**
- You want SQL-like syntax in TypeScript (what you write is approximately what runs)
- Bundle size matters (serverless, edge functions)
- You need RLS support for Supabase multi-tenancy
- You prefer code-first schema without codegen steps
- You want to stay close to raw SQL without losing type safety
- Cold start performance is critical

**Lean toward Prisma when:**
- You prefer a higher-level abstraction over SQL
- Your team is already productive with Prisma
- You value Prisma's mature tooling (Studio, Migrate, Accelerate)
- You're building a traditional server-rendered app (not edge-heavy)
- You want a larger ecosystem of guides, plugins, and community answers

**Neither is wrong.** Both are production-ready for TypeScript + Postgres. The choice depends on team preference, deployment target, and how close to SQL you want to work.

---

## Convex Data Patterns

**Convex is NOT a Postgres database. It is a reactive document database with its own TypeScript-native schema and query system. ORM concepts do not apply.**

### How Convex Differs

| Concept | Postgres (Supabase/Neon) | Convex |
|---------|-------------------------|--------|
| Schema definition | SQL DDL or ORM schema | `schema.ts` with TypeScript validators |
| Queries | SQL or ORM query builder | TypeScript `query` functions |
| Mutations | SQL INSERT/UPDATE/DELETE | TypeScript `mutation` functions |
| Migrations | SQL migration files | Automatic schema push (`convex dev`) |
| Real-time | Requires WebSocket layer | Built-in — queries auto-update |
| Auth | Built-in (Supabase) or BYO | BYO (Clerk, Auth0, Auth.js) |
| Transactions | Explicit SQL transactions | Automatic — every mutation is transactional |
| Indexes | Manual CREATE INDEX | Declared in schema, managed automatically |

### Convex Schema Definition

```typescript
// convex/schema.ts
import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  users: defineTable({
    email: v.string(),
    passwordHash: v.string(),
    emailVerified: v.boolean(),
    onboardingCompleted: v.boolean(),
    platformRole: v.optional(v.string()), // undefined for regular users, "platform_admin" for app operators
  }).index("by_email", ["email"]),

  profiles: defineTable({
    userId: v.id("users"),
    firstName: v.string(),
    lastName: v.string(),
    role: v.string(),
    phone: v.optional(v.string()),
    avatarUrl: v.optional(v.string()),
  }).index("by_user", ["userId"]),

  organizations: defineTable({
    name: v.string(),
    phone: v.string(),
    addressLine1: v.optional(v.string()),
    city: v.optional(v.string()),
    state: v.optional(v.string()),
    zip: v.optional(v.string()),
    country: v.string(),
    teamSize: v.number(),
    industry: v.optional(v.string()),
    // Healthcare specific
    npiNumber: v.optional(v.string()),
    practiceType: v.optional(v.string()),
    hipaaBaaAccepted: v.optional(v.boolean()),
  }),

  memberships: defineTable({
    userId: v.id("users"),
    organizationId: v.id("organizations"),
    role: v.string(), // "office_owner" | "office_manager" | "office_staff"
    invitedBy: v.optional(v.id("users")),
    acceptedAt: v.optional(v.number()), // Unix timestamp
  })
    .index("by_user", ["userId"])
    .index("by_org", ["organizationId"]),
});
```

### Convex Query Example

```typescript
// convex/users.ts
import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

export const getProfile = query({
  args: { userId: v.id("users") },
  handler: async (ctx, args) => {
    const profile = await ctx.db
      .query("profiles")
      .withIndex("by_user", (q) => q.eq("userId", args.userId))
      .unique();
    return profile;
  },
});

export const completeOnboarding = mutation({
  args: {
    userId: v.id("users"),
    firstName: v.string(),
    lastName: v.string(),
    role: v.string(),
    orgName: v.string(),
    orgPhone: v.string(),
    orgLocation: v.string(),
    teamSize: v.number(),
  },
  handler: async (ctx, args) => {
    // Create profile
    await ctx.db.insert("profiles", {
      userId: args.userId,
      firstName: args.firstName,
      lastName: args.lastName,
      role: args.role,
    });

    // Create organization
    const orgId = await ctx.db.insert("organizations", {
      name: args.orgName,
      phone: args.orgPhone,
      addressLine1: args.orgLocation,
      country: "US",
      teamSize: args.teamSize,
    });

    // Create membership
    await ctx.db.insert("memberships", {
      userId: args.userId,
      organizationId: orgId,
      role: "office_owner",
    });

    // Mark onboarding complete
    await ctx.db.patch(args.userId, { onboardingCompleted: true });
  },
});
```

### When Convex Fits vs When It Doesn't

**Convex fits well:**
- Real-time collaborative apps (multiplayer, live dashboards, chat)
- Rapid prototyping (zero config, instant deploys)
- Apps where reactivity is core (data changes auto-push to UI)
- TypeScript-heavy teams who prefer no SQL

**Convex may not fit:**
- Complex relational queries (multi-join analytics, reporting)
- Apps requiring direct SQL access or Postgres extensions
- Existing Postgres tooling dependencies (pg_dump, PostGIS, full-text search)
- Teams invested in SQL-based workflows
- Apps needing very large datasets (512 MB free tier is tight)

---

## Schema Implementation Examples

The SaaS Standards Phase 4 defines four core tables: `users`, `profiles`, `organizations`, `memberships`. Below are type-safe implementations for each ORM/platform.

### Drizzle Schema (Supabase or Neon)

```typescript
// src/db/schema.ts
import {
  pgTable, uuid, varchar, boolean, integer, timestamp, text,
} from "drizzle-orm/pg-core";

export const users = pgTable("users", {
  id: uuid("id").primaryKey().defaultRandom(),
  email: varchar("email", { length: 255 }).unique().notNull(),
  passwordHash: varchar("password_hash", { length: 255 }).notNull(),
  emailVerified: boolean("email_verified").default(false),
  onboardingCompleted: boolean("onboarding_completed").default(false),
  platformRole: varchar("platform_role", { length: 50 }), // NULL for regular users, 'platform_admin' for app operators
  createdAt: timestamp("created_at", { withTimezone: true }).defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true }).defaultNow(),
});

export const profiles = pgTable("profiles", {
  id: uuid("id").primaryKey().defaultRandom(),
  userId: uuid("user_id").references(() => users.id, { onDelete: "cascade" }),
  firstName: varchar("first_name", { length: 100 }).notNull(),
  lastName: varchar("last_name", { length: 100 }).notNull(),
  role: varchar("role", { length: 100 }).notNull(),
  phone: varchar("phone", { length: 20 }),
  avatarUrl: text("avatar_url"),
  createdAt: timestamp("created_at", { withTimezone: true }).defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true }).defaultNow(),
});

export const organizations = pgTable("organizations", {
  id: uuid("id").primaryKey().defaultRandom(),
  name: varchar("name", { length: 255 }).notNull(),
  phone: varchar("phone", { length: 20 }).notNull(),
  addressLine1: varchar("address_line1", { length: 255 }),
  city: varchar("city", { length: 100 }),
  state: varchar("state", { length: 50 }),
  zip: varchar("zip", { length: 20 }),
  country: varchar("country", { length: 100 }).default("US"),
  teamSize: integer("team_size").notNull(),
  industry: varchar("industry", { length: 100 }),
  // Healthcare specific
  npiNumber: varchar("npi_number", { length: 20 }),
  practiceType: varchar("practice_type", { length: 100 }),
  hipaaBaaAccepted: boolean("hipaa_baa_accepted").default(false),
  createdAt: timestamp("created_at", { withTimezone: true }).defaultNow(),
  updatedAt: timestamp("updated_at", { withTimezone: true }).defaultNow(),
});

export const memberships = pgTable("memberships", {
  id: uuid("id").primaryKey().defaultRandom(),
  userId: uuid("user_id").references(() => users.id, { onDelete: "cascade" }),
  organizationId: uuid("organization_id").references(() => organizations.id, { onDelete: "cascade" }),
  role: varchar("role", { length: 50 }).notNull().default("office_staff"), // office_owner, office_manager, office_staff
  invitedBy: uuid("invited_by").references(() => users.id),
  acceptedAt: timestamp("accepted_at", { withTimezone: true }),
  createdAt: timestamp("created_at", { withTimezone: true }).defaultNow(),
});
```

### Drizzle Connection (Supabase)

```typescript
// src/db/index.ts
import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import * as schema from "./schema";

const client = postgres(process.env.DATABASE_URL!, { prepare: false });
export const db = drizzle(client, { schema });
```

### Drizzle Connection (Neon)

```typescript
// src/db/index.ts
import { drizzle } from "drizzle-orm/neon-http";
import { neon } from "@neondatabase/serverless";
import * as schema from "./schema";

const sql = neon(process.env.DATABASE_URL!);
export const db = drizzle(sql, { schema });
```

### Prisma Schema (Supabase or Neon)

```prisma
// prisma/schema.prisma
generator client {
  provider = "prisma-client-js"
}

datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

model User {
  id                  String       @id @default(uuid()) @db.Uuid
  email               String       @unique @db.VarChar(255)
  passwordHash        String       @map("password_hash") @db.VarChar(255)
  emailVerified       Boolean      @default(false) @map("email_verified")
  onboardingCompleted Boolean      @default(false) @map("onboarding_completed")
  platformRole        String?      @map("platform_role") @db.VarChar(50) // NULL for regular users, 'platform_admin' for app operators
  createdAt           DateTime     @default(now()) @map("created_at") @db.Timestamptz
  updatedAt           DateTime     @updatedAt @map("updated_at") @db.Timestamptz
  profile             Profile?
  memberships         Membership[] @relation("UserMemberships")
  invitations         Membership[] @relation("UserInvitations")

  @@map("users")
}

model Profile {
  id        String   @id @default(uuid()) @db.Uuid
  userId    String   @unique @map("user_id") @db.Uuid
  firstName String   @map("first_name") @db.VarChar(100)
  lastName  String   @map("last_name") @db.VarChar(100)
  role      String   @db.VarChar(100)
  phone     String?  @db.VarChar(20)
  avatarUrl String?  @map("avatar_url")
  createdAt DateTime @default(now()) @map("created_at") @db.Timestamptz
  updatedAt DateTime @updatedAt @map("updated_at") @db.Timestamptz
  user      User     @relation(fields: [userId], references: [id], onDelete: Cascade)

  @@map("profiles")
}

model Organization {
  id               String       @id @default(uuid()) @db.Uuid
  name             String       @db.VarChar(255)
  phone            String       @db.VarChar(20)
  addressLine1     String?      @map("address_line1") @db.VarChar(255)
  city             String?      @db.VarChar(100)
  state            String?      @db.VarChar(50)
  zip              String?      @db.VarChar(20)
  country          String       @default("US") @db.VarChar(100)
  teamSize         Int          @map("team_size")
  industry         String?      @db.VarChar(100)
  npiNumber        String?      @map("npi_number") @db.VarChar(20)
  practiceType     String?      @map("practice_type") @db.VarChar(100)
  hipaaBaaAccepted Boolean      @default(false) @map("hipaa_baa_accepted")
  createdAt        DateTime     @default(now()) @map("created_at") @db.Timestamptz
  updatedAt        DateTime     @updatedAt @map("updated_at") @db.Timestamptz
  memberships      Membership[]

  @@map("organizations")
}

model Membership {
  id             String       @id @default(uuid()) @db.Uuid
  userId         String       @map("user_id") @db.Uuid
  organizationId String       @map("organization_id") @db.Uuid
  role           String       @default("office_staff") @db.VarChar(50)
  invitedById    String?      @map("invited_by") @db.Uuid
  acceptedAt     DateTime?    @map("accepted_at") @db.Timestamptz
  createdAt      DateTime     @default(now()) @map("created_at") @db.Timestamptz
  user           User         @relation("UserMemberships", fields: [userId], references: [id], onDelete: Cascade)
  organization   Organization @relation(fields: [organizationId], references: [id], onDelete: Cascade)
  invitedBy      User?        @relation("UserInvitations", fields: [invitedById], references: [id])

  @@map("memberships")
}
```

### Convex Schema

See "Convex Data Patterns" section above for complete `schema.ts` matching Phase 4 tables.

---

## Migration Strategy

### Drizzle Kit (Supabase or Neon)

```bash
# Generate migration from schema changes
npx drizzle-kit generate

# Apply migrations
npx drizzle-kit migrate

# Push schema directly (dev only — skips migration files)
npx drizzle-kit push

# Pull existing database schema into Drizzle format
npx drizzle-kit pull

# Open Drizzle Studio for visual database management
npx drizzle-kit studio
```

**With Supabase:** Drizzle migrations coexist with Supabase's own migration system. Use `drizzle-kit generate` for schema changes and apply via `drizzle-kit migrate` or integrate into Supabase's `supabase/migrations/` folder.

**With Neon:** Use Neon's branching feature to test migrations on a branch before applying to main. Create a branch, apply migration, verify, merge.

### Prisma Migrate (Supabase or Neon)

```bash
# Create migration from schema changes
npx prisma migrate dev --name descriptive_name

# Apply pending migrations (production)
npx prisma migrate deploy

# Reset database (dev only)
npx prisma migrate reset

# Pull existing database into Prisma schema
npx prisma db pull

# Generate Prisma Client after schema changes
npx prisma generate
```

**With Supabase:** Prisma Migrate manages its own migration history table (`_prisma_migrations`). This is separate from Supabase's migration tracking. Don't mix both systems.

**With Neon:** Use Neon branching to test migrations safely. `prisma migrate dev` on a branch, verify, then `prisma migrate deploy` on production branch.

### Convex Schema Push

```bash
# Development: watches for changes, auto-pushes schema + functions
npx convex dev

# Production: deploy schema + functions
npx convex deploy
```

Convex handles schema changes automatically. When you modify `schema.ts` and run `convex dev`, it validates changes and applies them. For breaking changes (removing required fields, changing types), Convex will warn and require explicit handling.

---

## Multi-Tenancy Patterns

### Two-Layer Role Model (Platform + Office)

SaaS applications need two distinct authorization layers. Conflating them into a single `role` column breaks multi-office logic and creates privilege escalation risks.

**Layer 1 — Platform Role (global, internal app ops):**

| Role | Scope | Purpose |
|------|-------|---------|
| `platform_admin` | Global — all offices | App operator, infrastructure, emergency access |
| `NULL` | N/A | Regular user (default) |

Stored on `users.platform_role`. Only the developer/operator and emergency backup accounts get this. Regular customers never have a platform role.

**Layer 2 — Office Role (tenant-scoped, for customers):**

| Role | Scope | Purpose |
|------|-------|---------|
| `office_owner` | One office | Highest permission — billing, transfer ownership, delete office |
| `office_manager` | One office | Day-to-day ops — manage users, settings, data (not ownership actions) |
| `office_staff` | One office | Normal usage — read/write based on job needs |

Stored on `memberships.role`. A user can have different roles in different offices (multi-office support).

**Permission intent:**

| Actor | Can do |
|-------|--------|
| `platform_admin` | Anything across all offices (god mode) |
| `office_owner` | Manage that office's users, settings, data, billing, ownership |
| `office_manager` | Manage day-to-day data/users in that office (NOT ownership transfer, delete, billing) |
| `office_staff` | Limited read/write based on job function |

**Solo Developer Setup (recommended):**

As the app operator, create yourself as a regular app user, then set `platform_role = 'platform_admin'`. This way you dogfood your own product as a real user while retaining platform-level access for debugging, support, and emergency operations. Never give `platform_admin` to customers.

---

### Helper Functions (PostgreSQL)

RLS policies should call helper functions instead of self-querying the `users` table inside `users` policies (which causes circular reference issues).

```sql
-- Check if the current user is a platform admin
CREATE OR REPLACE FUNCTION is_platform_admin()
RETURNS BOOLEAN AS $$
  SELECT EXISTS (
    SELECT 1 FROM users
    WHERE id = auth.uid()
      AND platform_role = 'platform_admin'
  );
$$ LANGUAGE sql SECURITY DEFINER STABLE;

-- Check if the current user has a specific office role
CREATE OR REPLACE FUNCTION has_office_role(
  _office_id UUID,
  _allowed_roles TEXT[]
)
RETURNS BOOLEAN AS $$
  SELECT EXISTS (
    SELECT 1 FROM memberships
    WHERE user_id = auth.uid()
      AND organization_id = _office_id
      AND role = ANY(_allowed_roles)
  );
$$ LANGUAGE sql SECURITY DEFINER STABLE;
```

**Usage in RLS policies:** Policies call these helpers, keeping policy definitions clean and avoiding circular queries.

---

### Postgres (Supabase/Neon) — Row-Level Security

RLS ensures users only access their organization's data. Platform admins bypass tenant isolation for support/debugging.

**Supabase RLS Example (using helpers):**

```sql
-- Enable RLS on all tenant-scoped tables
ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE memberships ENABLE ROW LEVEL SECURITY;

-- Platform admins see everything
CREATE POLICY "platform_admin_full_access"
  ON organizations FOR ALL
  USING (is_platform_admin());

-- Regular users see only their organizations
CREATE POLICY "users_see_own_organizations"
  ON organizations FOR SELECT
  USING (
    id IN (
      SELECT organization_id FROM memberships
      WHERE user_id = auth.uid()
    )
  );

-- Only office_owner and office_manager can update organization settings
CREATE POLICY "managers_update_organizations"
  ON organizations FOR UPDATE
  USING (
    has_office_role(id, ARRAY['office_owner', 'office_manager'])
  );

-- Only office_owner can delete an organization
CREATE POLICY "owner_delete_organization"
  ON organizations FOR DELETE
  USING (
    has_office_role(id, ARRAY['office_owner'])
  );
```

**Drizzle RLS (v1.0+):**

```typescript
import { pgPolicy } from "drizzle-orm/pg-core";
import { sql } from "drizzle-orm";

export const organizations = pgTable("organizations", {
  // ... columns as defined in schema above
}, (table) => [
  pgPolicy("platform_admin_full_access", {
    for: "all",
    using: sql`is_platform_admin()`,
  }),
  pgPolicy("users_see_own_orgs", {
    for: "select",
    using: sql`id IN (
      SELECT organization_id FROM memberships
      WHERE user_id = auth.uid()
    )`,
  }),
  pgPolicy("managers_update_orgs", {
    for: "update",
    using: sql`has_office_role(id, ARRAY['office_owner', 'office_manager'])`,
  }),
  pgPolicy("owner_delete_org", {
    for: "delete",
    using: sql`has_office_role(id, ARRAY['office_owner'])`,
  }),
]);
```

### Convex — Function-Level Access Control

Convex doesn't have RLS. The two-layer role model is enforced directly in query/mutation functions:

```typescript
// Helper: check platform admin status
async function isPlatformAdmin(ctx: QueryCtx, userId: string): Promise<boolean> {
  const user = await ctx.db.get(userId as Id<"users">);
  return user?.platformRole === "platform_admin";
}

// Helper: check office role
async function hasOfficeRole(
  ctx: QueryCtx,
  userId: string,
  orgId: Id<"organizations">,
  allowedRoles: string[]
): Promise<boolean> {
  const membership = await ctx.db
    .query("memberships")
    .withIndex("by_user", (q) => q.eq("userId", userId as Id<"users">))
    .collect();
  return membership.some(
    (m) => m.organizationId === orgId && allowedRoles.includes(m.role)
  );
}

export const getOrganization = query({
  args: { orgId: v.id("organizations") },
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) throw new Error("Unauthenticated");

    const userId = identity.subject;

    // Platform admins can access any organization
    if (await isPlatformAdmin(ctx, userId)) {
      return await ctx.db.get(args.orgId);
    }

    // Regular users must be a member
    const isMember = await hasOfficeRole(
      ctx, userId, args.orgId,
      ["office_owner", "office_manager", "office_staff"]
    );
    if (!isMember) throw new Error("Not a member of this organization");

    return await ctx.db.get(args.orgId);
  },
});
```

---

*Last updated: 2026-02-03. Free tier data verified against official pricing pages.*
*Sources: supabase.com/pricing, neon.tech/pricing, convex.dev/pricing, orm.drizzle.team, prisma.io/blog*
