# CI/CD Guardrails Templates

Templates and strategies for enforcing repo structure via local hooks and CI pipelines.

---

## Local Hook Strategy

### pre-push (Recommended Default)

**When to use:** Most projects. Catches issues before code reaches remote.

**Behavior:**
- Run repo health checks
- Block push on FAIL severity
- Print warnings for WARN severity with suggested actions
- Allow proceed after warnings

**Trade-offs:**
| Aspect | pro-push | pre-commit |
|--------|----------|------------|
| Feedback speed | Slower (runs on push) | Faster (runs on commit) |
| Developer friction | Lower (batch checks) | Higher (every commit) |
| Safety | Higher (last gate) | Lower (can be skipped with --no-verify) |
| Best for | Structure checks, tests | Formatting, linting |

### pre-commit (Alternative)

**When to use:** Fast checks only (formatting, linting). Not recommended for structure checks.

**Behavior:**
- Run lightweight checks (lint, format)
- Block commit on failure
- Keep execution under 5 seconds

### Hook Installation

```bash
#!/bin/bash
# scripts/install-hooks.sh

HOOKS_DIR=".git/hooks"

# Install pre-push hook
cat > "$HOOKS_DIR/pre-push" << 'HOOK'
#!/bin/bash
echo "Running repo health checks..."
./scripts/check-structure.sh
exit $?
HOOK

chmod +x "$HOOKS_DIR/pre-push"
echo "Hooks installed."
```

---

## CI Strategy (GitHub Actions)

### Complete Workflow Template

```yaml
name: Repo Health

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  pull-requests: write

jobs:
  structure-check:
    name: Structure Validation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run structure checks
        id: checks
        run: |
          chmod +x ./scripts/check-structure.sh
          ./scripts/check-structure.sh --json > reports/health.json 2>&1
        continue-on-error: true

      - name: Upload health report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: repo-health-report
          path: reports/health.json
          retention-days: 30

      - name: Annotate failures
        if: steps.checks.outcome == 'failure'
        run: |
          echo "::error::Repo structure validation failed. See health report artifact for details."
          # Parse JSON report and create annotations
          if [ -f reports/health.json ]; then
            jq -r '.checks[] | select(.status == "FAIL") | "::error::\(.category)/\(.name): \(.message). Fix: \(.fix // "See documentation")"' reports/health.json
          fi
          exit 1

      - name: Report warnings
        if: steps.checks.outcome == 'success'
        run: |
          if [ -f reports/health.json ]; then
            jq -r '.checks[] | select(.status == "WARN") | "::warning::\(.category)/\(.name): \(.message)"' reports/health.json 2>/dev/null || true
          fi
```

---

## check-structure.sh Skeleton

```bash
#!/bin/bash
# scripts/check-structure.sh
# Validates repo structure against configured framework rules.
#
# Usage:
#   ./scripts/check-structure.sh           # Human-readable output
#   ./scripts/check-structure.sh --json    # Machine-readable JSON
#
# Exit codes:
#   0 - All checks pass (WARN is OK)
#   1 - One or more FAIL checks

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────

# Customize these for your project:
ALLOWED_TOP_LEVEL=("src" "tests" "docs" "scripts" ".github" "public")
REQUIRED_FILES=("README.md" ".gitignore")
MAX_DEPTH=4
MISC_THRESHOLD=15  # percentage

JSON_MODE=false
[[ "${1:-}" == "--json" ]] && JSON_MODE=true

# ─── Check Functions ────────────────────────────────────────────────────

FAILS=0
WARNS=0
CHECKS=()

check() {
  local category="$1" name="$2" severity="$3" status="$4" message="$5"
  if [[ "$severity" == "FAIL" && "$status" == "FAIL" ]]; then
    ((FAILS++))
  elif [[ "$severity" == "WARN" && "$status" == "FAIL" ]]; then
    ((WARNS++))
    status="WARN"
  fi
  CHECKS+=("{\"category\":\"$category\",\"name\":\"$name\",\"severity\":\"$severity\",\"status\":\"$status\",\"message\":\"$message\"}")
}

# ─── Structural Checks ─────────────────────────────────────────────────

# Check allowed top-level folders
for dir in */; do
  dir="${dir%/}"
  [[ "$dir" == "node_modules" || "$dir" == ".git" || "$dir" =~ ^\. ]] && continue
  found=false
  for allowed in "${ALLOWED_TOP_LEVEL[@]}"; do
    [[ "$dir" == "$allowed" ]] && found=true && break
  done
  if ! $found; then
    check "structural" "allowed-top-level" "FAIL" "FAIL" "Unexpected top-level directory: $dir"
  fi
done

# Check required files
for file in "${REQUIRED_FILES[@]}"; do
  if [[ ! -f "$file" ]]; then
    check "structural" "required-files" "FAIL" "FAIL" "Missing required file: $file"
  else
    check "structural" "required-files" "FAIL" "PASS" "Found required file: $file"
  fi
done

# ─── Output ─────────────────────────────────────────────────────────────

TOTAL=${#CHECKS[@]}
PASS=$((TOTAL - FAILS - WARNS))

if $JSON_MODE; then
  mkdir -p reports
  echo "{\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"summary\":{\"total_checks\":$TOTAL,\"pass\":$PASS,\"fail\":$FAILS,\"warn\":$WARNS},\"checks\":[$(IFS=,; echo "${CHECKS[*]}")]}" | jq .
else
  echo "═══ Repo Health Check ═══"
  echo "Checks: $TOTAL | Pass: $PASS | Fail: $FAILS | Warn: $WARNS"
  echo ""
  for c in "${CHECKS[@]}"; do
    echo "$c" | jq -r '"[\(.severity)] \(.category)/\(.name): \(.message)"' 2>/dev/null || echo "$c"
  done
fi

[[ $FAILS -gt 0 ]] && exit 1
exit 0
```

---

## Failure Handling

### Error Message Format

```
[FAIL] structural/allowed-top-level: Unexpected top-level directory: tmp/
  Fix: Move tmp/ contents to an appropriate subdirectory or add to .gitignore
  Docs: See repo conventions in CONTRIBUTING.md

[WARN] structural/no-junk-root: Found stray file in root: notes.txt
  Fix: Move to docs/ or remove from tracking

[PASS] structural/required-files: All required files present
```

### Common Failure Remediation

| Failure | Cause | Fix |
|---------|-------|-----|
| Unexpected top-level dir | Ad-hoc folder creation | Move to correct parent or update allowed list |
| Missing required file | Forgotten during setup | Create file using scaffold script |
| Misc overflow | Lazy categorization | Reclassify items into proper topics |
| Stale index entries | Files moved without reindexing | Run reindex script |
| Depth exceeded | Over-nesting | Flatten hierarchy, use metadata instead |
