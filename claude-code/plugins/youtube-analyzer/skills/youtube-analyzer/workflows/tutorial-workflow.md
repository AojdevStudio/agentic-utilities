# Tutorial Workflow

Specialized analysis workflow for tutorial and course content. Extends general analysis with package tracking, GitHub integration, and production readiness assessment.

## Input

Same as GeneralWorkflow, plus:
- `config.repoUrl` — optional GitHub repository URL
- `config.focusArea` — one of: "production-checklist", "tool-inventory", "architecture-patterns", "step-by-step"

## Analysis Protocol

### Phase 1: Content Analysis
1. Read transcript chunk from file path
2. Identify tutorial structure: what is being built, in what order
3. Map the architecture/stack being taught
4. Note all commands, code snippets, and configurations mentioned

### Phase 2: Package & Framework Inventory
For every tool, package, framework, or library mentioned:
- Name (exact package name, e.g., "next" not "Next.js")
- Display name (human-readable, e.g., "Next.js")
- Version mentioned in video (if stated)
- Category: framework | library | tool | service | language | database
- How it's used in the tutorial
- Whether it's a core dependency or optional

### Phase 3: Production Readiness Assessment
Evaluate what the tutorial DOESN'T cover but a production app needs:
- Authentication & authorization
- Rate limiting & throttling
- Error handling & logging
- Input validation & sanitization
- Database migrations & seeding
- Environment configuration
- CI/CD pipeline
- Testing (unit, integration, e2e)
- Monitoring & observability
- Security headers & CORS
- Performance optimization
- Accessibility
- SEO (if web app)

### Phase 4: GitHub Repo Analysis Integration

If `repoExploreResults` is provided by the orchestrator (from Phase 3 repo exploration), the synthesis agent integrates these Mermaid diagrams into the final output.

**What the synthesis agent receives:**
- `repoExploreResults.structure` - Mermaid graph TD of project file organization
- `repoExploreResults.dependencies` - Mermaid graph LR of dependency tree with exact versions
- `repoExploreResults.patterns` - Mermaid sequence/flow diagrams of implementation patterns
- `repoExploreResults.summary` - Brief text overview

**What the synthesis agent does:**
1. Adds a "Ground Truth Architecture" section to the output with embedded Mermaid diagrams
2. Cross-references repo structure against tutorial content to identify discrepancies
3. Uses actual package versions from repo's package.json/lock file (more reliable than transcript mentions)
4. Notes any code patterns in the repo that the tutorial didn't cover
5. Lists production features present in repo but not taught in video

**Discrepancy examples:**
- Tutorial shows Pages Router but repo uses App Router
- Tutorial installs v14 but repo's lock file has v15
- Repo has auth middleware the tutorial never explains
- Repo has tests the tutorial skipped

See `workflows/repo-exploration.md` for the full agent prompt specifications.

## Output Modes

### Production Checklist Mode (config.focusArea = "production-checklist")
Focus on what's missing for production:
- Checklist of production requirements
- What the tutorial covers vs what's needed
- Recommended tools/packages for each gap
- Priority order for implementation

### Tool Inventory Mode (config.focusArea = "tool-inventory")
Focus on the technology stack:
- Complete package inventory with versions
- Alternative tools for each (with pros/cons)
- Deprecated or outdated packages flagged
- Ecosystem compatibility notes

### Architecture Patterns Mode (config.focusArea = "architecture-patterns")
Focus on design decisions:
- Architecture diagrams (described in text)
- Design patterns used
- Data flow mapping
- State management approach
- API design patterns

### Step-by-Step Mode (config.focusArea = "step-by-step")
Focus on following along:
- Numbered step sequence with commands
- Configuration files content
- Key decision points explained
- Common errors and fixes
- Checkpoint verification steps

## Output Structure

```markdown
## Executive Summary
[What this tutorial builds and teaches]

## Architecture Overview
[Stack diagram, data flow, key components]

## Package & Tool Inventory

| Package | Version (Video) | Category | Role in Tutorial |
|---------|----------------|----------|------------------|
| next | 14.2 | Framework | Core framework |
| prisma | 5.x | ORM | Database access |

## Step-by-Step Summary
1. [Step with key command]
2. [Step with key command]

## Production Readiness Checklist
- [ ] Authentication system — not covered
- [x] Database setup — covered with Prisma
- [ ] Rate limiting — not covered

## Key Patterns & Techniques
### [Pattern Name]
[Explanation with code reference]

## Tool Comparison
| Tool Used | Alternatives | Trade-offs |
|-----------|-------------|------------|
| Prisma | Drizzle, TypeORM | Type-safety vs flexibility |

## Quality Assessment
- Tutorial quality: [EXCELLENT/GOOD/FAIR]
- Code quality: [assessment]
- Production readiness: [LOW/MEDIUM/HIGH]
```

## Package Database Integration

After analysis, the synthesis agent will call the package database script for each package found:
```bash
bun run ${CLAUDE_PLUGIN_ROOT}/scripts/package-db.ts add \
  --name "next" \
  --display-name "Next.js" \
  --version-mentioned "14.2" \
  --category "framework" \
  --source "VIDEO_URL"
```

## Chunk-Specific Instructions

Same as GeneralWorkflow — analyze only your assigned chunk. The synthesis agent merges.
For package inventory: list ALL packages mentioned in your chunk, even if you suspect they appear in other chunks too. Deduplication happens at synthesis.
