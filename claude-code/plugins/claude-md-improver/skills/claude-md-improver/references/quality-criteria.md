# Quality Criteria

## Scoring Rubric

### 1. Project Intent & Conventions (25 points)

Agents need to understand *how the project thinks* — architectural constraints, convention rationale, workflow patterns, and boundaries. This is the highest-value content because agents cannot infer intent from code alone.

**25 points**: Clear architectural intent, convention rationale, workflow patterns, boundaries
- Why patterns exist, not just that they do
- Constraints explained with reasoning ("RLS from day one because multi-tenant security")
- Workflow sequences documented ("import → preview → mapping → commit → undo")

**18 points**: Good conventions documented, some missing rationale

**10 points**: Basic patterns listed without reasoning

**5 points**: Sparse or generic conventions

**0 points**: No intent or convention documentation

### 2. Anti-Staleness (25 points)

The #1 cause of misleading agent instructions is stale data — hardcoded counts, directory trees, and command lists that drift with every PR. This criterion rewards files that avoid volatile data and penalizes those that embed it.

**25 points**: Zero staleness risk
- No hardcoded file/directory counts
- No duplicated package.json scripts
- No directory trees that will drift
- Volatile info handled via pointers ("see package.json") or discovery ("agents can glob for this")

**18 points**: Minimal staleness risk (1-2 minor items)

**10 points**: Several items that will go stale (counts, partial listings)

**5 points**: Significant staleness — directory trees, command inventories, version numbers

**0 points**: Actively misleading — references to files/paths that no longer exist

### 3. Leanness (20 points)

Context is a scarce resource. Every token in the instruction file competes with the task, the code, and relevant docs. Lean files let agents focus on the work.

**20 points**: Under ~150 lines.  Every line teaches something an agent can't discover. No filler, no redundancy. Dense signal.

**15 points**: Under ~200 lines. Mostly lean, some padding.

**10 points**: 200-400 lines. Contains discoverable info that should be cut or pointed.

**5 points**: 400+ lines. Monolithic — crowds out task context.

**0 points**: 600+ lines or mostly filler/generic advice.

### 4. Non-Obvious Gotchas (15 points)

The highest-ROI content: concrete past failures, workarounds, edge cases, and "why we do it this weird way" explanations. Each gotcha should trace to a real problem.

**15 points**: Rich failure-driven gotchas
- Each item addresses a concrete past failure
- Workarounds explained with context
- Edge cases agents would hit documented
- "Why not X?" for unintuitive choices

**10 points**: Some gotchas documented, some generic

**5 points**: Minimal or theoretical gotchas

**0 points**: No gotchas or warnings

### 5. Discoverability & Pointers (15 points)

Instead of containing everything, great instruction files teach agents *where to look*. Pointers to source-of-truth files, config, and docs directories let agents load context on demand.

**15 points**: Active pointer strategy
- References specific files for deeper context ("see `.planning/STATE.md` for decision log")
- Volatile info delegated to source files ("commands in package.json", "env vars in .env.example")
- For large projects, docs/ directory recommended and linked

**10 points**: Some pointers, some inline duplication

**5 points**: Mostly self-contained, few external references

**0 points**: Monolithic — tries to contain everything

## Assessment Process

1. Read the instruction file completely
2. Cross-reference with actual codebase:
   - Check if referenced files still exist
   - Verify architectural descriptions match reality
   - Identify content that duplicates discoverable info
3. Score each criterion
4. Calculate total and assign grade
5. Flag specific staleness risks
6. Propose cuts (remove), points (redirect), and adds (new signal)

## Red Flags (Immediate Score Reduction)

- **Hardcoded counts** that will drift ("22 migration files", "15 API endpoints")
- **Command inventories** duplicating package.json scripts
- **Full directory trees** that change with every feature
- **References to deleted/renamed files** — actively misleading
- **Generic best practices** not specific to the project
- **Tech stack version numbers** that will become stale
- **Verbose explanations** of things the class/function name already says
- **Duplicate content** across multiple CLAUDE.md files in the same repo

## Green Flags (Score Boosters)

- **"Because" statements** explaining why conventions exist
- **Failure-driven gotchas** tracing to real incidents
- **Pointers to source-of-truth** instead of inline duplication
- **Explicit boundaries** ("never do X", "always Y before Z")
- **Status context** ("v1.0 shipped, planning v2") that orients the agent
- **Lean line count** relative to project complexity
