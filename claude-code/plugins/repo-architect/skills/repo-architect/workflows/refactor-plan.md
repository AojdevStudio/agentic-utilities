# RefactorPlan Workflow

**Trigger:** "refactor repo", "restructure", "move map"

**Context to load:** `move-map-spec.md`, `frameworks.md`, `health-checks.md`

---

## Workflow Steps

### Step 1: Capture Current State

If coming from AuditRepo, reuse the scan results. Otherwise, scan fresh:

1. **Scan structure** (Glob, 2-3 levels)
2. **Classify framework** (match against `frameworks.md` archetypes)
3. **Identify issues** (run `health-checks.md` checks)
4. **Count files and imports** to estimate migration scope

**Capture into session state:**
```json
{
  "project_path": "/path/to/project",
  "current_framework": "ad-hoc",
  "current_tree": ["src/", "config/", "utils/", "test/"],
  "file_count": 145,
  "import_pattern": "relative",
  "issues": ["junk root", "no shared dir", "tests not co-located"]
}
```

---

### Step 2: Select Target Framework

Use `frameworks.md` selection heuristics to determine the ideal target framework.

**Ask the user:**
```
question: "What framework should we restructure toward?"
header: "Target"
options:
  - label: "[Best match] (Recommended)"
    description: "[Why this fits based on their project]"
  - label: "[Alternative 1]"
    description: "[Trade-off explanation]"
  - label: "[Alternative 2]"
    description: "[Trade-off explanation]"
  - label: "Show me the ideal tree first"
    description: "Generate target tree before I commit to a framework"
```

If user picks "Show me the ideal tree first" — generate the tree for the top recommendation and let them review before proceeding.

---

### Step 3: Generate Target State

Produce the ideal directory tree for the selected framework, customized for their project:

```
project/                        [target]
├── src/                        [exists → keep]
│   ├── features/               [create]
│   │   ├── auth/               [move from src/auth/]
│   │   └── dashboard/          [move from src/pages/dashboard/]
│   └── shared/                 [create, move from src/utils/]
├── tests/                      [exists → restructure]
│   ├── unit/                   [create]
│   └── e2e/                    [create]
├── docs/                       [create]
├── scripts/                    [create]
│   └── check-structure.sh      [create]
├── README.md                   [exists → keep]
├── .env.example                [create]
└── .gitignore                  [exists → update]
```

---

### Step 4: Generate Move Map

Load `move-map-spec.md` and diff current vs target to produce the Move Map.

**For each difference:**
1. Determine old path → new path
2. Categorize the change (imports, configs, docs, ci, docker, scripts)
3. Estimate impact (number of files referencing old path)

**Output Move Map table:**

| # | Old Path | New Path | Category | Impact | Notes |
|---|----------|----------|----------|--------|-------|
| 1 | `src/utils/` | `src/shared/utils/` | imports | 23 files | Update import paths |
| 2 | `src/auth/` | `src/features/auth/` | imports | 8 files | Move entire module |
| 3 | `config/` | `ops/config/` | configs | 4 files | Update .env references |
| ... | ... | ... | ... | ... | ... |

---

### Step 5: Phase the Migration

Follow the 6-phase order from `move-map-spec.md`:

1. **Phase 1: Create target structure** — New directories only
2. **Phase 2: Move non-code files** — Docs, configs, README
3. **Phase 3: Move source code** — One module at a time
4. **Phase 4: Update all references** — Imports, configs, docs
5. **Phase 5: Update CI/CD & scripts** — Pipelines, Dockerfiles
6. **Phase 6: Enable enforcement** — Health checks, hooks

**For each phase, specify:**
- Moves included (reference Move Map #s)
- Verification: what to check after this phase
- Rollback: git revert strategy

---

### Step 6: Import/Path Change Analysis

Run detection to identify all files that will need path updates:

```bash
# For each move in the map:
rg "from.*old/path" --type ts -l  # Import statements
rg "old/path" *.json *.yaml       # Config references
rg "old/path" *.md                # Documentation links
rg "old/path" .github/            # CI paths
```

Report total impact:
- X import statements across Y files
- Z config references across W files
- N documentation links

---

### Step 7: Rollback Strategy

From `move-map-spec.md`:
- Git branch per phase
- Clear commit messages
- Emergency revert commands

---

### Step 8: Present and Ask (Agent-Aware)

Before presenting options, detect the current agent to determine available execution strategies:

```bash
bun run tools/detect-agent.ts
```

**Options vary by agent:**

**When agent is `claude-code` (default — can delegate):**
```
question: "The refactor plan is ready. How would you like to proceed?"
header: "Execute"
options:
  - label: "Execute via Codex (Recommended)"
    description: "Delegate execution to Codex CLI in phased steps with verification"
  - label: "Execute directly"
    description: "Execute the Move Map in the current session without delegation"
  - label: "Modify the plan first"
    description: "I want to adjust the Move Map before executing"
  - label: "Save plan only"
    description: "Save the plan for later execution — don't change anything now"
```

**When agent is `codex-cli` (self — cannot delegate, would be circular):**
```
question: "The refactor plan is ready. How would you like to proceed?"
header: "Execute"
options:
  - label: "Execute directly (Recommended)"
    description: "Execute the Move Map now in phased steps with verification"
  - label: "Modify the plan first"
    description: "I want to adjust the Move Map before executing"
  - label: "Save plan only"
    description: "Save the plan for later execution — don't change anything now"
```

**When agent is `gemini-cli` or `unknown`:**
```
question: "The refactor plan is ready. How would you like to proceed?"
header: "Execute"
options:
  - label: "Execute directly (Recommended)"
    description: "Execute the Move Map in the current session in phased steps"
  - label: "Delegate to Codex"
    description: "Hand off execution to Codex CLI for headless phased execution"
  - label: "Modify the plan first"
    description: "I want to adjust the Move Map before executing"
  - label: "Save plan only"
    description: "Save the plan for later execution — don't change anything now"
```

**Routing:**
- **"Execute via Codex"** or **"Delegate to Codex":** Write session state and route to `execute-refactor.md` (Step 3a)
- **"Execute directly":** Write session state and route to `execute-refactor.md` (Step 3b)
- **"Modify first":** Let user adjust, then re-present
- **"Save plan only":** Write Move Map to `docs/refactor-plan.md` in the project

---

## Session State Output

Write to `.dev/repo-architect-session.json` in the project root:

```json
{
  "project_path": "/path/to/project",
  "framework": "feature-based",
  "expected_dirs": ["src", "tests", "docs", "scripts"],
  "move_map": [
    { "from": "src/utils/", "to": "src/shared/utils/", "category": "imports" },
    { "from": "config/", "to": "ops/config/", "category": "configs" }
  ],
  "phase": "plan",
  "updated_at": "ISO-8601"
}
```

This state is read by the ExecuteRefactor workflow for phase coordination.
