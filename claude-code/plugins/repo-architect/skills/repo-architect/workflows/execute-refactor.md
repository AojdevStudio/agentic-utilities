# ExecuteRefactor Workflow

**Trigger:** "execute refactor", "run migration", "apply move map"

**Context to load:** `move-map-spec.md`

---

## Prerequisites

- A Move Map must exist (from RefactorPlan workflow or user-provided)
- Session state at `.dev/repo-architect-session.json` must contain `move_map` and `framework`
- If no Move Map exists, route to `refactor-plan.md` first

---

## Workflow Steps

### Step 0: Detect Current Agent

Before any execution, determine which agent is running this workflow to select the right execution strategy.

```bash
bun run tools/detect-agent.ts
```

**Routing logic:**

| Agent | Execution Strategy |
|-------|-------------------|
| `claude-code` | Delegate to Codex CLI via `codex exec` (Step 3a) |
| `codex-cli` | Execute moves directly — you ARE Codex, no delegation (Step 3b) |
| `gemini-cli` | Execute moves directly or delegate to Codex (Step 3a or 3b) |
| `unknown` | Execute moves directly (Step 3b) — safest default |

**The critical rule:** If the detected agent is `codex-cli`, NEVER call `codex exec`. That would be the agent delegating to itself, creating an infinite loop. Instead, execute the Move Map instructions directly as the current agent.

---

### Step 1: Verify Move Map Exists

1. Read `.dev/repo-architect-session.json`
2. Confirm `move_map` array is populated
3. Confirm `project_path` points to a valid directory
4. Confirm `framework` is set

**If missing:** Inform user and offer to run RefactorPlan first.

---

### Step 2: Group Moves into Phases

Following the 6-phase order from `move-map-spec.md`, group the Move Map entries:

| Phase | Moves | Description |
|-------|-------|-------------|
| 1 | — | Create target directories (no moves) |
| 2 | Docs, configs, non-code | Move non-code files |
| 3 | Source code by module | Move source files, one group at a time |
| 4 | — | Update all import/path references |
| 5 | CI, Docker, scripts | Update pipeline and build paths |
| 6 | — | Enable health checks and hooks |

---

### Step 3: Execute Each Phase

The execution strategy depends on the agent detected in Step 0.

#### Step 3a: Delegate to Codex CLI (when agent is `claude-code` or `gemini-cli`)

For each phase, construct a Codex prompt and delegate execution to Codex CLI in headless mode.

**Execution command:**
```bash
codex exec \
  --sandbox workspace-write \
  --full-auto \
  --skip-git-repo-check \
  2>/dev/null
```

**Prompt template per phase (from `move-map-spec.md`):**

```
You are executing Phase {N} of a repository restructuring.

PROJECT: {project_path}
FRAMEWORK: {target_framework}

MOVE MAP FOR THIS PHASE:
{move_map_table_for_this_phase}

INSTRUCTIONS:
1. Execute each move in the map above
2. After moving files, update all references:
   - Import statements that reference old paths
   - Configuration files with old paths
   - Documentation links
3. Do NOT modify any business logic — only paths and references
4. After all moves, verify:
   - No remaining references to old paths
   - All imports resolve correctly
   - Project still compiles/lints

CONSTRAINTS:
- Move files using git mv (preserves history)
- One logical group at a time
- Stop and report if any test fails
```

#### Step 3b: Execute Directly (when agent is `codex-cli` or `unknown`)

When the current agent IS Codex (or unknown), execute the Move Map instructions directly instead of delegating. This avoids circular delegation.

**For each phase, the current agent must:**

1. Read the Move Map entries for this phase
2. Execute file moves using `git mv` (preserves history)
3. Update all import/path references using search-and-replace:
   ```bash
   # For each move: find references to old path and update
   rg "old/path" --type ts --type tsx --type js -l | while read f; do
     sed -i '' "s|old/path|new/path|g" "$f"
   done
   ```
4. Update config files, documentation links, CI paths
5. Verify no remaining references to old paths
6. Run tests if available

**The key difference from 3a:** No subprocess spawning, no `codex exec`. The agent reading these instructions IS the executor. Follow the same constraints (git mv, one group at a time, stop on test failure) but execute them directly.

---

### Step 4: Verify After Each Phase

After each execution:

1. **Check files moved:** Verify old paths don't exist, new paths do
2. **Check references:** `rg "old/path"` should return zero results for moved items
3. **Run tests:** If project has tests, run them
4. **Run health checks:** Apply applicable checks from `health-checks.md`

**If verification fails:**
- Report which moves failed and why
- Offer rollback: `git revert HEAD` for this phase
- Ask whether to continue or stop

**If verification passes:**
- Commit phase: `git commit -m "refactor(structure): phase {N} — {description}"`
- Update session state with completed phase
- Proceed to next phase

---

### Step 5: Final Health Check

After all phases complete:

1. Run the full health check suite (all applicable categories)
2. Generate final JSON report
3. Compare against pre-refactor state (if captured by AuditRepo)

---

### Step 6: Report Results

**Output:**

```
## Refactor Execution Report

### Phases Completed: {N}/6

| Phase | Status | Moves | Issues |
|-------|--------|-------|--------|
| 1. Create structure | ✅ | — | — |
| 2. Non-code files | ✅ | 5 | — |
| 3. Source code | ✅ | 12 | 1 stale import fixed |
| 4. Update references | ✅ | 23 refs | — |
| 5. CI/CD & scripts | ✅ | 3 | — |
| 6. Enforcement | ✅ | — | — |

### Health Score
- Before: 8/18 (44%)
- After: 17/18 (94%)

### Remaining Issues
- [WARN] One documentation link not yet updated (low priority)

### Commits Created
- abc1234: refactor(structure): phase 1 — create target directories
- def5678: refactor(structure): phase 2 — move non-code files
- ...
```

---

### Step 7: Update Session State

Update `.dev/repo-architect-session.json`:

```json
{
  "project_path": "/path/to/project",
  "framework": "feature-based",
  "expected_dirs": ["src", "tests", "docs", "scripts"],
  "move_map": [...],
  "phase": "complete",
  "health_score_before": "8/18",
  "health_score_after": "17/18",
  "updated_at": "ISO-8601"
}
```

---

## Error Recovery

| Error | Response |
|-------|----------|
| Codex fails to execute | Report error, offer manual execution instructions |
| Tests fail after move | Revert phase, report failing tests, ask user |
| Stale imports found | Run targeted fix for just the stale refs |
| Permission denied | Report, suggest running with appropriate permissions |
| Move Map is empty | Route to RefactorPlan workflow |

---

## Codex CLI Dependency

This workflow uses Codex CLI for delegation (Step 3a) but does NOT require it when executing directly (Step 3b).

**Dependency check (only needed for Step 3a):**
1. Check: `which codex` or `codex --version`
2. If missing AND agent needs delegation: inform user and fall back to Step 3b (direct execution)
3. If missing AND agent is `codex-cli`: no issue — direct execution is the default path

**Agent-aware fallback chain:**
1. Preferred: delegate to Codex CLI (Step 3a) — when available and not circular
2. Fallback: execute directly (Step 3b) — always available
3. Last resort: export plan as task list for manual execution
