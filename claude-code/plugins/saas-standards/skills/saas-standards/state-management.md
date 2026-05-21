# State Management Standards for SaaS Applications

**Context file — Reference for state management architecture decisions in SaaS apps.**

---

## The Core Principle

**State management is not one problem — it's five.**

Most teams pick a library first, then try to fit all their state into it. This creates unnecessary complexity. The correct approach: identify WHERE your state lives, THEN choose the right tool for each layer.

---

## The Five State Layers

Every SaaS application has up to five distinct state layers. Each has different characteristics, different tools, and different trade-offs.

### Layer 1: Server State (Database/API)

**Definition:** Data that lives authoritatively in a database or external API. The client has a COPY, not the source of truth.

**Examples:** User records, claims, invoices, team members, settings, audit logs

**Characteristics:**
- Source of truth is the server
- Client copy can become stale
- Needs cache invalidation strategy
- Benefits from deduplication (don't fetch same data twice)
- May need optimistic updates (show change immediately, sync later)
- May need background refetching (keep data fresh)

**Tools:**

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **TanStack Query** | Most SaaS apps with REST/GraphQL APIs | Query deduplication, cache, optimistic mutations, prefetch hydration |
| **SWR** | Simpler apps, Vercel ecosystem | Lightweight, stale-while-revalidate pattern |
| **Apollo Client** | GraphQL-heavy apps | Normalized cache, GraphQL subscriptions |
| **Server Components** (Next.js) | Server-rendered pages, simple CRUD | Zero client JS, automatic deduplication |
| **Supabase Realtime** | Supabase-backed apps needing live updates | Direct database change subscriptions |

**Decision Framework:**

```
Is your data fetched from a server/database?
├── YES → This is server state
│   ├── Using Next.js App Router with Server Components?
│   │   ├── Simple CRUD, no optimistic updates needed?
│   │   │   └── Server Components + Server Actions + revalidatePath (zero library)
│   │   └── Need caching, optimistic updates, or real-time?
│   │       └── TanStack Query (or SWR) for client components
│   ├── Using React SPA / Pages Router?
│   │   └── TanStack Query (default) or SWR (simpler)
│   └── Using GraphQL?
│       └── Apollo Client or TanStack Query + graphql-request
└── NO → Not server state (see other layers)
```

**Why TanStack Query over Zustand for server state:**

| Capability | TanStack Query | Zustand |
|------------|---------------|---------|
| Query deduplication | Built-in — same query in 5 components = 1 fetch | Manual — you build this yourself |
| Cache invalidation | `invalidateQueries()` — declarative | Manual — you track what to refetch |
| Optimistic updates | `onMutate` + automatic rollback on error | Manual — you build rollback logic |
| Background refetching | `refetchInterval`, `refetchOnWindowFocus` | Manual — you set up intervals |
| Prefetching | `prefetchQuery()` for SSR hydration | Not applicable |
| Stale time management | Per-query `staleTime` / `gcTime` config | Not applicable |
| Retry logic | Automatic with exponential backoff | Manual |
| Pagination | `useInfiniteQuery` built-in | Manual |
| Devtools | Dedicated query devtools | Redux DevTools (different focus) |

Zustand stores client state. TanStack Query manages server state. They solve different problems.

---

### Layer 2: Client State (UI-Only)

**Definition:** State that exists ONLY in the browser. No server equivalent. Ephemeral or session-scoped.

**Examples:** Sidebar collapsed, modal open, wizard step, bulk selection, theme preference, notification queue

**Characteristics:**
- No server counterpart
- Dies on page refresh (unless persisted to localStorage)
- Shared across components (not just one component)
- Changes don't trigger API calls

**Tools:**

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **Zustand** | Most client state needs | Tiny (~1KB), no boilerplate, middleware (persist, devtools) |
| **Jotai** | Atomic/granular reactivity | Bottom-up atoms, no selectors needed |
| **Redux Toolkit** | Large teams, complex client logic | Predictable, extensive ecosystem |
| **React Context** | Compound components, theme/locale | Built-in, no dependency |
| **useState** | Single-component state | Simplest possible, no overhead |

**Decision Framework:**

```
Is this state used by only ONE component?
├── YES → useState (don't over-engineer)
└── NO → Is it shared across 2+ disconnected components?
    ├── YES → Do you need middleware (persist, devtools, immer)?
    │   ├── YES → Zustand (lightweight + middleware) or Redux Toolkit (complex)
    │   └── NO → Zustand or Jotai (both work, Jotai for atomic patterns)
    └── NO → Is it parent-child prop drilling (2-3 levels)?
        ├── Tolerable (2 levels) → Props are fine
        └── Painful (3+ levels) → Zustand or Context
```

---

### Layer 3: URL State (Shareable/Bookmarkable)

**Definition:** State encoded in the URL. Shareable, bookmarkable, survives page refresh.

**Examples:** Table filters, sort order, search query, pagination, selected tab, active view

**Characteristics:**
- Must survive page refresh
- Must be shareable (send link to colleague → same view)
- Changes trigger navigation/server re-render
- Limited by URL length (~2000 chars)

**Tools:**

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **nuqs** | Type-safe URL state in Next.js | Serializers, shallow routing, search params |
| **useSearchParams** (Next.js) | Simple URL params | Built-in, no dependency |
| **react-router** search params | React Router apps | Standard URL state management |
| **Custom hooks** | Specific URL patterns | Full control |

**Decision Framework:**

```
Should this state be in the URL?
├── User sends link → colleague sees same view? → YES, URL state
├── User refreshes → should state persist? → YES, URL state
├── Is it filter/sort/search/pagination? → YES, URL state
└── Is it ephemeral (modal open, tooltip visible)? → NO, not URL state
```

---

### Layer 4: Form State (Input Management)

**Definition:** State inside forms — field values, validation, dirty tracking, submission.

**Examples:** Registration form, claim edit form, onboarding wizard inputs, settings page

**Characteristics:**
- Temporary until submitted
- Needs validation (client + server)
- May need dirty tracking (unsaved changes warning)
- Complex forms benefit from schema validation (Zod)

**Tools:**

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **React Hook Form** | Most form scenarios | Performance (uncontrolled), Zod integration |
| **Formik** | Legacy projects | Mature ecosystem |
| **useActionState** (React 19) | Server Action forms | Built-in, progressive enhancement |
| **Conform** | Server-first form validation | Works with Server Actions natively |
| **useState** | Simple 1-2 field forms | No dependency needed |

**Decision Framework:**

```
How complex is the form?
├── 1-2 fields → useState (don't add a library)
├── 3-10 fields → React Hook Form + Zod
├── Multi-step wizard → React Hook Form + Zod + useReducer for step state
└── Server-first (progressive enhancement) → useActionState or Conform
```

---

### Layer 5: Real-Time State (Live Updates)

**Definition:** State pushed FROM the server TO the client without the client asking. Events, notifications, presence.

**Examples:** Live activity feed, notification badges, online presence, live dashboards

**Characteristics:**
- Server-initiated, not client-initiated
- Needs connection management (reconnect, heartbeat)
- May need to update multiple disconnected components
- Often combined with server state (invalidate query cache on event)

**Tools:**

| Tool | Best For | Key Strength |
|------|----------|-------------|
| **Supabase Realtime** | Supabase-backed apps | Direct DB change subscriptions, Presence |
| **Pusher** | Any backend, managed service | Reliable, channels, presence |
| **Socket.io** | Custom WebSocket server | Flexibility, rooms, namespaces |
| **Server-Sent Events (SSE)** | One-way server→client | Simple, HTTP-based, no library needed |
| **Ably** | Enterprise real-time | Guaranteed delivery, history |

**Integration with other layers:**

```
Real-time event arrives (e.g., "record_updated")
├── Using TanStack Query? → invalidateQueries(['records', recordId])
│   └── Query refetches automatically, UI updates
├── Using Zustand? → store.updateRecord(event.data)
│   └── All subscribed components re-render
├── Using Server Components? → router.refresh() or revalidatePath()
│   └── Server re-renders with fresh data
```

---

## Common SaaS Patterns

### Pattern 1: Next.js App Router + Supabase

```
┌─────────────────────────────────────────────────────┐
│ SERVER STATE  │ TanStack Query (or Server Components │
│               │ for simple pages)                     │
│               │ Supabase queries, mutations           │
├───────────────┼──────────────────────────────────────┤
│ URL STATE     │ useSearchParams or nuqs               │
│               │ Filters, sort, search, pagination     │
├───────────────┼──────────────────────────────────────┤
│ CLIENT STATE  │ Zustand (if cross-component needed)   │
│               │ Wizard state, bulk selection, toasts   │
├───────────────┼──────────────────────────────────────┤
│ FORM STATE    │ React Hook Form + Zod                 │
│               │ Onboarding wizard, record editing      │
├───────────────┼──────────────────────────────────────┤
│ REAL-TIME     │ Supabase Realtime                     │
│               │ Record updates, presence, notifications│
└───────────────┴──────────────────────────────────────┘
```

### Pattern 2: React SPA + REST API

```
┌─────────────────────────────────────────────────────┐
│ SERVER STATE  │ TanStack Query                       │
├───────────────┼──────────────────────────────────────┤
│ URL STATE     │ React Router search params            │
├───────────────┼──────────────────────────────────────┤
│ CLIENT STATE  │ Zustand                               │
├───────────────┼──────────────────────────────────────┤
│ FORM STATE    │ React Hook Form + Zod                 │
├───────────────┼──────────────────────────────────────┤
│ REAL-TIME     │ Socket.io or Pusher                   │
└───────────────┴──────────────────────────────────────┘
```

### Pattern 3: Next.js Pages Router (Legacy / Tutorial Pattern)

```
┌─────────────────────────────────────────────────────┐
│ SERVER STATE  │ TanStack Query or SWR                 │
├───────────────┼──────────────────────────────────────┤
│ URL STATE     │ next/router query params              │
├───────────────┼──────────────────────────────────────┤
│ CLIENT STATE  │ Zustand (common choice)               │
├───────────────┼──────────────────────────────────────┤
│ FORM STATE    │ React Hook Form + Zod                 │
├───────────────┼──────────────────────────────────────┤
│ REAL-TIME     │ Pusher or Socket.io                   │
└───────────────┴──────────────────────────────────────┘
```

---

## Anti-Patterns

### 1. Using a Client State Library for Server State

**Wrong:** Put API data in Zustand/Redux, manually fetch/cache/invalidate.
**Right:** Use TanStack Query — it handles caching, deduplication, and invalidation.

### 2. Using a Server State Library for Client State

**Wrong:** Put UI state (modal open, sidebar collapsed) in TanStack Query.
**Right:** Use useState, Zustand, or Jotai for client-only state.

### 3. Putting Everything in One Global Store

**Wrong:** One massive Redux store with user data, form state, UI toggles, and cached API responses.
**Right:** Separate tools for separate concerns. Server state ≠ client state ≠ form state.

### 4. Avoiding All Libraries on Principle

**Wrong:** "We don't need libraries" — then building ad-hoc caching, deduplication, and optimistic update logic across 50 components.
**Right:** Evaluate where complexity is growing. If you're reimplementing what a library does, use the library.

### 5. Adding Libraries Preemptively Without Need

**Wrong:** Day 1 of a project, install Redux + React Query + Zustand + Jotai "just in case."
**Right:** Start with React primitives. Add libraries when specific pain points emerge or are clearly on the roadmap.

---

## The Architectural Runway Test

For each state layer, ask: "Is this capability on my roadmap within 6 months?"

| Capability | If YES (planned) | If NO (hypothetical) |
|------------|-------------------|----------------------|
| Server cache + optimistic updates | Add TanStack Query now | Keep Server Components |
| Cross-component client state | Add Zustand now | Keep useState |
| Real-time updates | Add Supabase Realtime / Pusher now | Skip |
| Complex multi-step forms | Add React Hook Form + Zod now | Keep useActionState |
| Type-safe URL state | Add nuqs now | Keep useSearchParams |

**The cost curve principle:** Infrastructure patterns are cheaper to add early than to retrofit later. But "early" means "when it's on the roadmap," not "day one just in case."

---

## Healthcare/Dental SaaS Considerations

### HIPAA-Sensitive State Rules

| State Layer | HIPAA Consideration |
|-------------|---------------------|
| Server State (TanStack Query) | Cache contains PHI — ensure `gcTime` is reasonable, clear on logout |
| Client State (Zustand) | NEVER store PHI in client stores. Store IDs/references only |
| URL State | NEVER put patient names, DOB, SSN in URL params |
| Form State | Clear form state on navigation away from patient context |
| Real-Time | Encrypt WebSocket payloads if transmitting PHI |
| localStorage / persist | NEVER persist PHI to localStorage or sessionStorage |

### Cache Clearing on Logout

```typescript
// MANDATORY for healthcare SaaS
function handleLogout() {
  queryClient.clear()           // Clear TanStack Query cache (may contain PHI)
  useAppStore.getState().reset() // Clear Zustand stores
  sessionStorage.clear()         // Clear session storage
  // DO NOT rely on garbage collection for PHI data
}
```

---

## Migration Paths

### From Zero-Library to TanStack Query

**When:** Your Server Components + revalidatePath pattern is creating UX friction (loading spinners on every mutation, stale data after cross-tab edits).

**Steps:**
1. Install: `bun add @tanstack/react-query`
2. Add QueryClientProvider to root layout
3. Migrate ONE data-heavy page first
4. Replace Server Component fetch with `useQuery`
5. Replace Server Action mutation with `useMutation` + `invalidateQueries`
6. Verify: faster perceived performance, no full-page reloads on mutations

### From Zero-Library to Zustand

**When:** You have 3+ components sharing non-server state (wizard state, bulk selection, notification queue) and prop drilling is painful.

**Steps:**
1. Install: `bun add zustand`
2. Create ONE store for the most painful case (e.g., an ImportWizard)
3. Move useState hooks into store
4. Replace prop drilling with store hooks
5. Verify: cleaner components, debuggable with Redux DevTools

### Combining TanStack Query + Zustand

**When:** You need BOTH server state management AND cross-component client state.

**Rule:** They don't overlap. TanStack Query owns server data. Zustand owns client-only UI state.

```typescript
// Server state — TanStack Query
const { data: records } = useQuery({
  queryKey: ['records', orgId],
  queryFn: () => getRecordsForOrg(orgId),
})

// Client state — Zustand
const { selectedRecords, toggleSelection } = useSelectionStore()

// They compose, they don't compete
```
