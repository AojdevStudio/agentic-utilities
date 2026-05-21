# Repo Organization Frameworks

8 framework archetypes for repository organization. Each framework defines a primary organizing principle, ideal use cases, and a skeleton directory tree.

---

## 1. Topic-Based

**Organizing principle:** Subject matter / domain area.

**Ideal for:** Knowledge bases, documentation repos, transcript archives, content libraries.

**Skeleton tree:**
```
repo/
├── topics/
│   ├── authentication/
│   ├── networking/
│   └── security/
├── shared/
├── scripts/
├── README.md
└── index.json
```

**Strengths:**
- Intuitive for browsing and discovery
- Maps naturally to how humans think about content
- Easy onboarding — folder names tell the story

**Weaknesses:**
- Cross-cutting concerns don't fit cleanly
- Can become deeply nested with sub-topics
- Requires discipline to avoid topic sprawl

**Anti-patterns:**
- Topics that overlap significantly (auth vs. security)
- Single-file topics (too granular)
- Topics named by format instead of subject ("pdfs/", "videos/")

---

## 2. Runtime-Boundary

**Organizing principle:** Execution context (runtime, build, ops, deploy).

**Ideal for:** Docker-based services, workflow automation, multi-runtime products.

**Skeleton tree:**
```
repo/
├── runtime/
│   ├── api/
│   ├── worker/
│   └── scheduler/
├── build/
│   ├── docker/
│   └── scripts/
├── ops/
│   ├── deploy/
│   ├── monitoring/
│   └── config/
├── docs/
├── tests/
└── README.md
```

**Strengths:**
- Clear separation between what runs, what builds, and what operates
- Docker/k8s configs live where they belong
- Operations team can navigate independently

**Weaknesses:**
- Shared code between runtimes needs explicit placement
- Can feel over-structured for small projects
- Feature changes touch multiple boundaries

**Anti-patterns:**
- Mixing runtime code with build tooling
- Ops configs scattered across runtime directories
- No clear boundary between dev and prod configs

---

## 3. Feature-Based

**Organizing principle:** User-facing capability / feature area.

**Ideal for:** Product applications with cohesive features, SPAs, mobile apps.

**Skeleton tree:**
```
repo/
├── src/
│   ├── features/
│   │   ├── auth/
│   │   │   ├── components/
│   │   │   ├── hooks/
│   │   │   ├── api/
│   │   │   └── tests/
│   │   ├── dashboard/
│   │   └── billing/
│   ├── shared/
│   │   ├── components/
│   │   ├── utils/
│   │   └── hooks/
│   └── app/
├── public/
├── tests/
│   └── e2e/
└── README.md
```

**Strengths:**
- Feature work is self-contained — all related code co-located
- Easy to delete/refactor a feature without affecting others
- New developers can focus on one feature at a time

**Weaknesses:**
- Shared code management requires discipline
- Cross-feature interactions can be unclear
- Feature boundaries may shift as product evolves

**Anti-patterns:**
- Features that depend heavily on each other's internals
- "shared" becoming a dumping ground
- Feature folders with only one file

#### Module-First Upgrade Path

When using Feature-Based for a SaaS app, `src/features/` SHOULD become `src/modules/` with deep module patterns:
- Each feature folder becomes a module with `index.ts` entrypoint
- Feature internals (components, hooks, utils) become `internal/`
- Feature API becomes orchestrators with IO types

---

## 4. Layered (Clean Architecture)

**Organizing principle:** Domain / application / infrastructure separation.

**Ideal for:** Long-lived backend systems, enterprise services, complex business logic.

**Skeleton tree:**
```
repo/
├── src/
│   ├── domain/
│   │   ├── entities/
│   │   ├── value-objects/
│   │   └── services/
│   ├── application/
│   │   ├── use-cases/
│   │   ├── ports/
│   │   └── dtos/
│   ├── infrastructure/
│   │   ├── database/
│   │   ├── http/
│   │   ├── messaging/
│   │   └── external-services/
│   └── presentation/
│       ├── controllers/
│       ├── middleware/
│       └── routes/
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── docs/
└── README.md
```

**Strengths:**
- Business logic is isolated and testable
- Infrastructure can be swapped without touching domain
- Clear dependency direction (domain has no external deps)

**Weaknesses:**
- More boilerplate and indirection
- Can feel over-engineered for simple CRUD
- Requires understanding of architectural boundaries

**Anti-patterns:**
- Domain layer importing from infrastructure
- Use cases that directly manipulate database
- Presentation logic in domain entities

#### Module-First Mapping

Layered architecture use-cases map directly to module orchestrators:
- `domain/` → `modules/<capability>/internal/domain.ts`
- `application/` (use-cases) → `modules/<capability>/orchestrators/`
- `infrastructure/` → `modules/<capability>/internal/providers/`
- The module `index.ts` is the boundary between layers

---

## 5. Package-Based (Monorepo)

**Organizing principle:** Deployable / publishable packages.

**Ideal for:** Multi-module codebases, shared libraries, design systems, microservices.

**Skeleton tree:**
```
repo/
├── packages/
│   ├── core/
│   │   ├── src/
│   │   ├── tests/
│   │   └── package.json
│   ├── api/
│   │   ├── src/
│   │   ├── tests/
│   │   └── package.json
│   ├── web/
│   │   ├── src/
│   │   ├── tests/
│   │   └── package.json
│   └── shared/
│       ├── src/
│       ├── tests/
│       └── package.json
├── tools/
├── docs/
├── package.json
├── turbo.json / nx.json / pnpm-workspace.yaml
└── README.md
```

**Strengths:**
- Independent versioning and deployment per package
- Shared code is explicitly packaged
- Tooling support (Turborepo, Nx, pnpm workspaces)

**Weaknesses:**
- Complex dependency management
- Build configuration overhead
- Cross-package changes require coordination

**Anti-patterns:**
- Packages with circular dependencies
- Single-file packages
- Shared package that every other package depends on (god package)

---

## 6. Data-Pipeline-Based

**Organizing principle:** Data flow stages (ingestion, processing, outputs).

**Ideal for:** ETL pipelines, ML workflows, RAG systems, data engineering.

**Skeleton tree:**
```
repo/
├── ingestion/
│   ├── sources/
│   ├── extractors/
│   └── validators/
├── processing/
│   ├── transforms/
│   ├── enrichment/
│   └── models/
├── outputs/
│   ├── exports/
│   ├── reports/
│   └── embeddings/
├── data/
│   ├── raw/
│   ├── processed/
│   └── artifacts/
├── pipelines/
│   └── configs/
├── tests/
├── scripts/
└── README.md
```

**Strengths:**
- Data flow is visible in the directory structure
- Easy to trace data lineage
- Stages can be developed and tested independently

**Weaknesses:**
- Shared utilities across stages need a home
- Not intuitive for non-data engineers
- Stage boundaries can be fuzzy

**Anti-patterns:**
- Processing logic in ingestion stage
- Raw data mixed with processed data
- No clear data versioning strategy

---

## 7. Index-First (Metadata-Driven)

**Organizing principle:** Intelligence lives in indexes and metadata, not folder hierarchy.

**Ideal for:** Large corpora, searchable archives, RAG knowledge bases, content management.

**Skeleton tree:**
```
repo/
├── content/
│   ├── 2024/
│   │   └── *.md (with frontmatter)
│   └── 2025/
│       └── *.md (with frontmatter)
├── indexes/
│   ├── master-index.json
│   ├── by-topic.json
│   ├── by-source.json
│   └── by-date.json
├── schemas/
│   ├── content-schema.json
│   └── index-schema.json
├── scripts/
│   ├── reindex.ts
│   ├── validate.ts
│   └── ingest.ts
├── tests/
└── README.md
```

**Strengths:**
- Scales to thousands of files without deep nesting
- Multiple access patterns via different indexes
- Metadata is explicit and searchable
- RAG/embedding-friendly

**Weaknesses:**
- Requires index maintenance tooling
- Index drift is a real risk
- Browsing the filesystem directly is less intuitive

**Anti-patterns:**
- Files without frontmatter/metadata
- Indexes that aren't auto-generated
- Folder hierarchy that duplicates index categories

---

## 8. Ops Hub

**Organizing principle:** Multi-workspace operational hub with centralized knowledge base.

**Ideal for:** Personal infrastructure repos, multi-domain ops centers, AI-assisted workspaces, knowledge-heavy private projects.

**Skeleton tree:**
```
repo/
├── workspace-a/          # Major domain workspace
│   ├── projects/
│   ├── tasks/
│   └── config/
├── workspace-b/          # Major domain workspace
│   ├── projects/
│   ├── tasks/
│   └── config/
├── knowledge/            # Cross-cutting knowledge base
│   ├── topic-a/
│   │   ├── subtopic/
│   │   └── *.md
│   ├── topic-b/
│   └── index.json
├── scripts/              # Automation and tooling
├── .dev/specs/           # Planning and specifications
├── tasks/                # General task tracking
├── .claude/              # AI configuration
├── CLAUDE.md
├── .gitignore
└── README.md
```

**Strengths:**
- Natural home for "everything about my operations" repos
- Supports deep knowledge nesting (8-12 levels) without health check warnings
- AI tooling configuration lives at root naturally
- Multiple independent workspaces coexist cleanly
- Typically private — no LICENSE/CONTRIBUTING pressure

**Weaknesses:**
- Can sprawl without clear workspace boundaries
- Knowledge directory needs pruning discipline
- Not a good fit for code that needs building/deploying
- Workspaces can become dumping grounds

**Anti-patterns:**
- Workspace directories that share heavy cross-dependencies
- Knowledge base without any organizational structure (flat dump)
- Mixing deployable code with documentation workspaces
- Using workspace directories for what should be separate repos

---

## Selection Heuristics

| Scenario | Recommended Framework | Runner-Up |
|----------|----------------------|-----------|
| Docker + service runtime | Runtime-Boundary | Layered |
| Corpus + search/embeddings | Index-First | Topic-Based + metadata |
| Product app (simple SPA/mobile) | Feature-Based | — |
| Product app (complex backend) | Layered | Feature-Based |
| Multiple deployables | Package-Based (Monorepo) | Runtime-Boundary |
| ETL / ML / data engineering | Data-Pipeline-Based | — |
| Knowledge base / docs | Topic-Based | Index-First |
| Exploratory / research | Topic-Based + sandbox | Data-Pipeline-Based |
| Multi-workspace operations hub | Ops Hub | Topic-Based |
| Personal AI-assisted infrastructure | Ops Hub | Index-First |

#### SaaS with Modules

When building a SaaS product with user accounts and multi-tenancy:
- **Primary:** Feature-Based (upgraded to Module-First) OR Layered
- **Secondary:** SaaSStandards skill for auth/onboarding/route guards
- **Combination:** Feature-Based + Module-First + SaaSStandards is the most common SaaS pattern

## Hybrid Patterns

Frameworks can be combined when a single framework doesn't fit:

| Combination | When to Use |
|-------------|-------------|
| Feature-Based + Layered | Product app with complex business logic per feature |
| Index-First + Topic-Based | Large corpus with both browsable hierarchy and searchable indexes |
| Package-Based + Runtime-Boundary | Monorepo with multiple services sharing packages |
| Data-Pipeline + Index-First | RAG system with data pipeline feeding indexed corpus |
| Ops Hub + Index-First | Ops hub with searchable knowledge base needing multiple access patterns |

---

### Module Directory Mapping

For any framework that includes `src/modules/`:

| Directory | Purpose |
|-----------|---------|
| `src/modules/` | Capability modules (deep modules) |
| `src/actions/` | Server Action wiring (Next.js App Router) |
| `src/ui/` or `app/` | UI components and pages |
| `src/runtime/` | Effect composition root (when applicable) |
| `src/lib/` | Minimal shared infra only (logging, env, DB connection) |
