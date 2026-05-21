# Workflow: AuditStateManagement

**Audit and recommend state management architecture for a SaaS application through structured interview.**

---

## Trigger

User wants to evaluate, set up, or improve state management in their SaaS app.

Examples:
- "Audit my state management"
- "What state management should I use?"
- "Set up state management for my app"
- "Do I need Zustand / TanStack Query / Redux?"
- "Review my app's state architecture"

---

## Philosophy

**This workflow is diagnostic, not prescriptive.** It does NOT default to any library. It interviews the developer to understand WHERE their state lives, WHAT problems they face, and THEN recommends the right tool for each layer.

**The five state layers (from state-management.md):**
1. Server State (database/API data)
2. Client State (UI-only, cross-component)
3. URL State (shareable, bookmarkable)
4. Form State (input management)
5. Real-Time State (live updates)

**Load reference before proceeding:**
Read `state-management.md`

---

## Execution Steps

### Step 1: Discover Current Architecture

Before interviewing, explore the codebase to understand what exists:

```
Glob: **/package.json — check for existing state libraries
Grep: "zustand|@tanstack/react-query|@tanstack/query|redux|jotai|recoil|swr|@apollo" — find state libraries
Grep: "useState" — count usage across app
Grep: "useSearchParams|useRouter|router.push" — find URL state patterns
Grep: "createContext|useContext" — find Context usage
Grep: "useActionState|useFormState" — find form patterns
Grep: "supabase.*realtime|pusher|socket.io|WebSocket" — find real-time patterns
```

Record findings for each layer:
- **Installed libraries:** (from package.json)
- **useState count:** X hooks across Y files
- **URL state files:** Z files using searchParams
- **Context providers:** N contexts
- **Real-time:** present/absent
- **Form handling:** pattern used

---

### Step 2: Interview — App Profile

Use AskUserQuestion to understand the application context.

**Question 1: Framework & Architecture**

```
AskUserQuestion:
  question: "What framework and routing pattern does your app use?"
  header: "Framework"
  options:
    - label: "Next.js App Router"
      description: "Server Components, Server Actions, app/ directory"
    - label: "Next.js Pages Router"
      description: "getServerSideProps, pages/ directory"
    - label: "React SPA (Vite/CRA)"
      description: "Client-side only, no SSR"
    - label: "Remix"
      description: "Loaders, actions, nested routing"
```

**Question 2: Primary Data Source**

```
AskUserQuestion:
  question: "Where does your app's data primarily live?"
  header: "Data source"
  options:
    - label: "Supabase"
      description: "PostgreSQL via Supabase client, may include Realtime"
    - label: "REST API"
      description: "Custom backend with REST endpoints"
    - label: "GraphQL"
      description: "GraphQL server (Apollo, Hasura, etc.)"
    - label: "Firebase / Firestore"
      description: "Google Firebase real-time database"
```

**Question 3: App Stage**

```
AskUserQuestion:
  question: "What stage is your application at?"
  header: "Stage"
  options:
    - label: "New project (greenfield)"
      description: "Starting from scratch, no existing patterns"
    - label: "Early prototype (< 10 pages)"
      description: "Working app but limited complexity"
    - label: "Growing app (10-30 pages)"
      description: "Multiple features, patterns emerging"
    - label: "Mature app (30+ pages)"
      description: "Established patterns, refactoring needed"
```

---

### Step 3: Interview — State Layer Assessment

For each of the five state layers, determine current state and needs.

**Question 4: Server State Pain Points**

```
AskUserQuestion:
  question: "Which server state problems are you experiencing or expect within 6 months?"
  header: "Server state"
  multiSelect: true
  options:
    - label: "Stale data after mutations"
      description: "User edits something but sees old data until page refresh"
    - label: "Redundant API calls"
      description: "Same data fetched multiple times across components"
    - label: "No optimistic updates"
      description: "UI waits for server response before showing changes"
    - label: "No issues currently"
      description: "Server Components / manual fetching works fine"
```

**Question 5: Client State Pain Points**

```
AskUserQuestion:
  question: "Which client-side state problems are you experiencing or expect within 6 months?"
  header: "Client state"
  multiSelect: true
  options:
    - label: "Prop drilling (3+ levels)"
      description: "Passing state through components that don't use it"
    - label: "Complex component state (5+ useState)"
      description: "Components with many useState hooks that are hard to manage"
    - label: "Cross-component sync needed"
      description: "Multiple disconnected components need shared state"
    - label: "No issues currently"
      description: "useState and local state work fine"
```

**Question 6: Real-Time & Collaboration Needs**

```
AskUserQuestion:
  question: "What real-time features do you need (now or within 6 months)?"
  header: "Real-time"
  multiSelect: true
  options:
    - label: "Live notifications"
      description: "Toast/badge updates when something changes"
    - label: "Multi-user collaboration"
      description: "Multiple users can view/edit same data simultaneously"
    - label: "Presence indicators"
      description: "Show who's online or viewing a record"
    - label: "None planned"
      description: "No real-time features on the roadmap"
```

**Question 7: Multi-Step Forms**

```
AskUserQuestion:
  question: "How complex are your forms?"
  header: "Forms"
  options:
    - label: "Simple (1-5 fields)"
      description: "Basic inputs, no wizards"
    - label: "Moderate (6-15 fields, some validation)"
      description: "Multiple sections, Zod/schema validation"
    - label: "Complex (multi-step wizards)"
      description: "Multi-step flows, conditional logic, draft saving"
    - label: "Mixed (some simple, some complex)"
      description: "Varies by feature"
```

---

### Step 4: Analyze & Diagnose

Based on interview answers, evaluate each state layer:

#### Server State Assessment

| Signal | Recommendation |
|--------|---------------|
| Next.js App Router + no pain points | Server Components + Server Actions (no library) |
| Next.js App Router + stale data or redundant calls | TanStack Query for client components |
| Next.js App Router + optimistic updates needed | TanStack Query with `useMutation` |
| React SPA or Pages Router | TanStack Query (default) or SWR (simpler) |
| GraphQL backend | Apollo Client or TanStack Query + graphql-request |
| Supabase + real-time needed | Supabase Realtime + TanStack Query for cache |

#### Client State Assessment

| Signal | Recommendation |
|--------|---------------|
| No cross-component state | useState is fine — don't add a library |
| Prop drilling pain (3+ levels) | Zustand (lightweight) or Context (if limited scope) |
| Complex component state (5+ useState) | Extract custom hook first. Zustand only if cross-component |
| Cross-component sync (notifications, selections) | Zustand with devtools middleware |
| Large team, complex client logic | Redux Toolkit (predictability at scale) |

#### URL State Assessment

| Signal | Recommendation |
|--------|---------------|
| Simple filters/search | useSearchParams (built-in) |
| Type-safe URL state with serialization | nuqs |
| React Router app | react-router search params |

#### Form State Assessment

| Signal | Recommendation |
|--------|---------------|
| Simple forms (1-5 fields) | useState or useActionState |
| Moderate forms with validation | React Hook Form + Zod |
| Multi-step wizards | React Hook Form + Zod + step state hook |
| Server-first progressive enhancement | Conform or useActionState |

#### Real-Time Assessment

| Signal | Recommendation |
|--------|---------------|
| Supabase backend | Supabase Realtime (channels + presence) |
| Custom backend | Socket.io or Pusher |
| Notifications only (one-way) | Server-Sent Events (SSE) |
| No real-time needed | Skip entirely |

---

### Step 5: Generate Recommendation

Produce a structured recommendation for each layer:

```markdown
## State Management Architecture Recommendation

### Your App Profile
- **Framework:** [answer]
- **Data source:** [answer]
- **Stage:** [answer]

### Recommended Architecture

| Layer | Tool | Justification |
|-------|------|---------------|
| Server State | [tool] | [why this tool for their situation] |
| Client State | [tool or "useState (no library)"] | [why] |
| URL State | [tool] | [why] |
| Form State | [tool] | [why] |
| Real-Time | [tool or "Not needed yet"] | [why] |

### Implementation Priority

1. **NOW:** [highest-impact layer to address first]
2. **SOON:** [second priority, within 1-2 sprints]
3. **LATER:** [when roadmap demands it]

### What NOT To Add

- [Library X] — [why it doesn't apply to their situation]

### Migration Path

If migrating from current patterns:
1. [First step — lowest risk, highest value]
2. [Second step — build on first]
3. [Third step — complete architecture]
```

---

### Step 6: Implementation Scaffolding

If the user approves the recommendation, set up the chosen tools:

#### For TanStack Query:

```bash
# Install
bun add @tanstack/react-query @tanstack/react-query-devtools
```

Create:
- `providers/QueryProvider.tsx` — QueryClientProvider wrapper
- `lib/queries/` — query key factories and query functions
- Wire into root layout

#### For Zustand:

```bash
# Install
bun add zustand
```

Create:
- `stores/` directory with first store
- Follow the specific use case identified in interview (wizard, selection, etc.)

#### For React Hook Form + Zod:

```bash
# Install
bun add react-hook-form @hookform/resolvers zod
```

Create:
- `lib/schemas/` — Zod schemas for forms
- Update identified complex forms to use React Hook Form

#### For Supabase Realtime:

Create:
- `lib/realtime/` — channel subscription hooks
- Integration with TanStack Query cache invalidation (if applicable)

---

### Step 7: Verification

After setup, verify the implementation:

| Check | Pass | Fail |
|-------|------|------|
| Chosen libraries installed correctly | In package.json, importable | Missing or version conflict |
| Provider wired into app root | Wraps all pages that need it | Missing or misplaced |
| First use case working | Data flows through chosen tool | Errors or fallback to old pattern |
| No layer confusion | Server state in server tool, client state in client tool | PHI in client store, API data in Zustand |
| DevTools accessible | Query/Redux devtools showing state | Not configured |
| Healthcare: PHI not in client stores | No patient data in Zustand/localStorage | PHI leaked to client state |
| Healthcare: Cache clears on logout | All caches cleared on sign-out | Stale PHI data persists |

---

## Output Format

```markdown
## State Management Audit Report

### App Profile
- Framework: [X]
- Data Source: [X]
- Stage: [X]

### Current State (Discovered)
| Layer | Current Approach | Issues Found |
|-------|-----------------|--------------|
| Server | [what exists] | [problems or "None"] |
| Client | [what exists] | [problems or "None"] |
| URL | [what exists] | [problems or "None"] |
| Forms | [what exists] | [problems or "None"] |
| Real-Time | [what exists] | [problems or "None"] |

### Recommended Architecture
| Layer | Recommendation | Priority |
|-------|---------------|----------|
| Server | [tool + justification] | [NOW/SOON/LATER] |
| Client | [tool + justification] | [NOW/SOON/LATER] |
| URL | [tool + justification] | [NOW/SOON/LATER] |
| Forms | [tool + justification] | [NOW/SOON/LATER] |
| Real-Time | [tool + justification] | [NOW/SOON/LATER] |

### Anti-Patterns to Avoid
1. [specific to their app]

### Next Steps
1. [actionable first step]
2. [second step]
3. [third step]
```
