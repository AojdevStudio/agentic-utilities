#!/usr/bin/env bash
# audit-hazards.sh — scan a repo for shared-state hazards that would collide
# if multiple git worktrees ran a live dev session at once.
#
# Usage: audit-hazards.sh [repo-path]
#   repo-path defaults to current working directory
#
# Output: structured report on stdout. Exit 0 always (this is advisory).
#
# Categories scanned:
#   1. Hardcoded ports in TS / JS / Rust / Python / Go
#   2. DB / data-file paths (sqlite, pg sockets, redis sockets, app data dirs)
#   3. Generic socket / IPC files
#   4. Build/cache dirs (confirmed in .gitignore — flag if tracked)
#   5. Submodules (each new worktree needs init)

set -euo pipefail

REPO="${1:-$PWD}"
REPO="$(cd "$REPO" && pwd)"

if ! git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ERROR: $REPO is not a git repo" >&2
  exit 2
fi

echo "═════════════════════════════════════════════════════════════════"
echo "  Multi-worktree hazard audit"
echo "  repo: $REPO"
echo "═════════════════════════════════════════════════════════════════"
echo

# ----------------------------------------------------------------------------
# Pick a search backend. rg is preferred (faster, smarter). Both branches
# define a `search` function that takes a regex and outputs `path:line:match`.
# ----------------------------------------------------------------------------
EXCLUDE_GLOBS=(
  '!node_modules/**'
  '!.git/**'
  '!target/**'
  '!dist/**'
  '!build/**'
  '!.next/**'
  '!.venv/**'
  '!__pycache__/**'
  '!*.lock'
  '!*.lockb'
  '!opensrc/**'
)

if command -v rg >/dev/null 2>&1; then
  search() {
    local pattern="$1"
    local args=(--no-heading --line-number --color=never --hidden)
    for g in "${EXCLUDE_GLOBS[@]}"; do
      args+=(--glob "$g")
    done
    # rg uses regex by default; pattern goes as positional after flags.
    rg "${args[@]}" -e "$pattern" "$REPO" 2>/dev/null || true
  }
else
  echo "ℹ️  ripgrep (rg) not found — falling back to grep. Install rg for better results."
  search() {
    local pattern="$1"
    grep -RInE \
      --exclude-dir=node_modules --exclude-dir=.git \
      --exclude-dir=target --exclude-dir=dist --exclude-dir=build \
      --exclude-dir=.next --exclude-dir=.venv --exclude-dir=__pycache__ \
      --exclude-dir=opensrc \
      --exclude='*.lock' --exclude='*.lockb' \
      "$pattern" "$REPO" 2>/dev/null || true
  }
fi

# Filter out test files, type decls, and our own gotcha doc references.
# CLAUDE.md often *describes* the very hazards we're scanning for (e.g. a
# hardcoded dev-server port), so exclude it to avoid false positives.
filter_noise() {
  grep -Ev '\.test\.|\.spec\.|/__tests__/|\.d\.ts:|/tests/|CLAUDE\.md:|AGENTS\.md:|README\.md:|/docs/|/threat-model|/HANDOFF' || true
}

# -----------------------------------------------------------------------------
# 1. Hardcoded ports
# -----------------------------------------------------------------------------
echo "━━━ 1. Hardcoded ports ━━━"
echo

PORT_PATTERNS=(
  'localhost:[0-9]{2,5}'
  ':3000([^0-9]|$)'
  ':3001([^0-9]|$)'
  ':5173([^0-9]|$)'
  ':8000([^0-9]|$)'
  ':8080([^0-9]|$)'
  ':1420([^0-9]|$)'
  '\.listen\([0-9]{4,5}'
  'PORT[ ]*\?\?[ ]*[0-9]{4,5}'
  'port[ ]*:[ ]*[0-9]{4,5}'
)

PORT_HITS=0
declare -a PORT_REPORT=()
for p in "${PORT_PATTERNS[@]}"; do
  hits=$(search "$p" | filter_noise | head -8)
  if [ -n "$hits" ]; then
    echo "  pattern: $p"
    echo "$hits" | sed 's|^|    |'
    echo
    count=$(echo "$hits" | wc -l | tr -d ' ')
    PORT_HITS=$((PORT_HITS + count))
  fi
done

if [ "$PORT_HITS" -eq 0 ]; then
  echo "  (none found — or already env-driven)"
  echo
fi

# -----------------------------------------------------------------------------
# 2. DB / data-file paths
# -----------------------------------------------------------------------------
echo "━━━ 2. DB / data-file paths ━━━"
echo

DB_PATTERNS=(
  '\.[a-z][a-z0-9_-]{2,}/data\.db'           # ~/.app/data.db (generic app data dir)
  'process\.env\.HOME[^,]{0,40}\.[a-z]'      # ~/.foo derived from $HOME
  'data\.db'
  '\.sqlite3?\b'
  'database\.url'
  'DB_PATH'
  'DATABASE_URL'
  'pgdata'
  '/var/run/[^"]*\.sock'
  '/tmp/[^"]*\.sock'
)

DB_HITS=0
for p in "${DB_PATTERNS[@]}"; do
  hits=$(search "$p" | filter_noise | head -6)
  if [ -n "$hits" ]; then
    echo "  pattern: $p"
    echo "$hits" | sed 's|^|    |'
    echo
    count=$(echo "$hits" | wc -l | tr -d ' ')
    DB_HITS=$((DB_HITS + count))
  fi
done

if [ "$DB_HITS" -eq 0 ]; then
  echo "  (none found — or already env-driven)"
  echo
fi

# -----------------------------------------------------------------------------
# 3. Socket / IPC files
# -----------------------------------------------------------------------------
echo "━━━ 3. Sockets / IPC ━━━"
echo

SOCK_PATTERNS=(
  '\.sock(\b|")'
  'unix:///'
  'createIPC'
  'createUnixSocket'
  'AF_UNIX'
)

SOCK_HITS=0
for p in "${SOCK_PATTERNS[@]}"; do
  hits=$(search "$p" | filter_noise | head -4)
  if [ -n "$hits" ]; then
    echo "  pattern: $p"
    echo "$hits" | sed 's|^|    |'
    echo
    count=$(echo "$hits" | wc -l | tr -d ' ')
    SOCK_HITS=$((SOCK_HITS + count))
  fi
done

if [ "$SOCK_HITS" -eq 0 ]; then
  echo "  (none found)"
  echo
fi

# -----------------------------------------------------------------------------
# 4. Build artifact dirs — should be gitignored, not tracked
# -----------------------------------------------------------------------------
echo "━━━ 4. Build artifact dirs (should be gitignored) ━━━"
echo

BUILD_DIRS=(node_modules target dist build .next .venv __pycache__ .turbo .svelte-kit)
TRACKED_BUILD=0
for d in "${BUILD_DIRS[@]}"; do
  if git -C "$REPO" ls-files | awk -F/ -v d="$d" '$1==d {found=1; exit} END{exit !found}' 2>/dev/null; then
    echo "  ⚠️  $d/ is TRACKED — should be in .gitignore (would be shared, not per-worktree)"
    TRACKED_BUILD=$((TRACKED_BUILD + 1))
  fi
done

if [ "$TRACKED_BUILD" -eq 0 ]; then
  echo "  (no build dirs tracked — all per-worktree as expected)"
  echo
fi

# -----------------------------------------------------------------------------
# 5. Submodules
# -----------------------------------------------------------------------------
echo "━━━ 5. Submodules ━━━"
echo

if [ -f "$REPO/.gitmodules" ]; then
  echo "  ⚠️  .gitmodules present — each new worktree will need:"
  echo "      git submodule update --init --recursive"
  echo
  echo "  Submodules declared:"
  grep '^\[submodule' "$REPO/.gitmodules" | sed 's|^|    |' || true
  echo
else
  echo "  (no submodules)"
  echo
fi

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------
echo "═════════════════════════════════════════════════════════════════"
echo "  Summary"
echo "═════════════════════════════════════════════════════════════════"
TOTAL=$((PORT_HITS + DB_HITS + SOCK_HITS + TRACKED_BUILD))
echo "  port hazards:    $PORT_HITS"
echo "  db/data hazards: $DB_HITS"
echo "  socket hazards:  $SOCK_HITS"
echo "  tracked build:   $TRACKED_BUILD"
echo

if [ "$TOTAL" -ge 3 ]; then
  echo "  → Recommend MODE A (code-only parallel)."
  echo "    Multiple shared-state handles are hardcoded; running parallel"
  echo "    live dev sessions would corrupt shared state."
elif [ "$TOTAL" -ge 1 ]; then
  echo "  → MODE A is the safe default."
  echo "    A few shared-state handles exist; Mode B is reachable with"
  echo "    limited code changes."
else
  echo "  → MODE B (full parallel dev) is likely already reachable."
  echo "    Few or no hardcoded shared-state handles found."
  echo "    Verify env-var coverage before relying on this."
fi
echo
