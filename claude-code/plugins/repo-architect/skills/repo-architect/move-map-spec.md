# Move Map Specification

The Move Map is the central artifact for repo refactoring. It maps every file/directory from its current location to its target location, with notes on what else needs updating.

---

## Move Map Table Format

| # | Old Path | New Path | Category | Notes |
|---|----------|----------|----------|-------|
| 1 | `src/utils/` | `src/shared/utils/` | imports | Update 12 import statements |
| 2 | `config/` | `ops/config/` | configs | Update .env references |
| 3 | `tests/` | `src/__tests__/` | imports | Co-locate with source |
| 4 | `Dockerfile` | `docker/Dockerfile` | docker | Update docker-compose.yml paths |
| 5 | `docs/api.md` | `docs/reference/api.md` | docs | Update README links |

---

## Path Change Categories

Each move falls into one or more categories that determine what else needs updating:

| Category | What to Update | Detection Method |
|----------|---------------|------------------|
| `imports` | Import/require statements in source files | `rg "from.*old/path" --type ts` |
| `configs` | Configuration files (.env, tsconfig, etc.) | `rg "old/path" *.json *.yaml *.toml` |
| `docs` | Documentation links and references | `rg "old/path" *.md` |
| `ci` | CI/CD pipeline paths | `rg "old/path" .github/` |
| `docker` | Dockerfile COPY/ADD paths, compose volumes | `rg "old/path" Dockerfile docker-compose*` |
| `scripts` | Shell scripts, package.json scripts | `rg "old/path" scripts/ package.json` |

---

## 6-Phase Minimal-Risk Migration Order

Execute moves in this order to minimize breakage window:

### Phase 1: Create Target Structure
- Create all new directories
- No files moved yet — zero breakage risk
- **Verify:** All target directories exist

### Phase 2: Move Non-Code Files
- Move docs, configs, README, CONTRIBUTING
- Low-risk — these don't affect runtime
- **Verify:** Non-code files accessible at new locations

### Phase 3: Move Source Code
- Move source files one logical group at a time
- Update imports within moved group immediately
- **Verify:** Each group compiles/lints after move

### Phase 4: Update All References
- Fix remaining import statements across codebase
- Update configuration file paths
- Update documentation links
- **Verify:** `rg "old/path"` returns zero results

### Phase 5: Update CI/CD & Scripts
- Update GitHub Actions workflow paths
- Update Dockerfile COPY/ADD paths
- Update package.json scripts
- Update shell scripts
- **Verify:** CI pipeline runs green

### Phase 6: Enable Enforcement
- Activate structure health checks
- Enable pre-push hooks
- Remove any temporary symlinks
- **Verify:** Full health check suite passes

---

## Rollback Strategy

### Git Branch Strategy
- Create a feature branch for each phase: `refactor/phase-1-structure`, etc.
- Each phase is a single commit (easy to revert)
- Merge phases into main one at a time

### Temporary Symlinks (Optional)
```bash
# During migration, keep old paths working via symlinks
ln -s new/path/utils old/path/utils
# Remove after all references are updated
```

### Commit Messages
Use clear, searchable commit messages:
```
refactor(structure): phase 1 — create target directories
refactor(structure): phase 2 — move non-code files
refactor(structure): phase 3 — move source code (auth module)
refactor(structure): phase 4 — update all import paths
refactor(structure): phase 5 — update CI/CD and scripts
refactor(structure): phase 6 — enable structure enforcement
```

### Emergency Rollback
```bash
# Revert the last phase
git revert HEAD

# Revert all phases back to pre-refactor
git revert --no-commit HEAD~3..HEAD
git commit -m "revert: rollback structure refactor (phases 4-6)"
```

---

## Validation Checklist

After each phase, verify:

- [ ] All files moved to new locations
- [ ] All imports updated and resolving
- [ ] All tests passing
- [ ] CI pipeline green (if applicable)
- [ ] No stale references: `rg "old/path"` returns 0 results
- [ ] Documentation links working
- [ ] Docker builds succeed (if applicable)
- [ ] Team notified of structure changes

After all phases complete:

- [ ] Full health check suite passes
- [ ] No temporary symlinks remaining
- [ ] Pre-push hooks enabled and passing
- [ ] CONTRIBUTING.md updated with new structure
- [ ] README.md reflects new directory layout

---

## Codex Prompt Template

Used by ExecuteRefactor workflow when delegating to Codex CLI:

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
