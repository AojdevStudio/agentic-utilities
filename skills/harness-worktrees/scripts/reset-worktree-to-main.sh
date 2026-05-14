#!/usr/bin/env bash
set -euo pipefail

REMOTE="origin"
BRANCH="main"
CONFIRM=0
STASH=0

usage() {
  cat <<'USAGE'
reset-worktree-to-main.sh — reset the current git worktree branch to latest origin/main

Dry-run by default. This intentionally does not switch to main, because main may
be checked out in another worktree. It resets the current branch to origin/main.

Usage:
  reset-worktree-to-main.sh [--remote origin] [--branch main]
  reset-worktree-to-main.sh --confirm [--remote origin] [--branch main]
  reset-worktree-to-main.sh --confirm --stash [--remote origin] [--branch main]

Options:
  --confirm          Execute the reset. Without this flag, only prints a plan.
  --stash            Before reset, stash tracked + untracked local changes.
                     Requires --confirm. Without --stash, dirty trees abort.
  --remote NAME      Remote to fetch/reset from. Default: origin.
  --branch NAME      Branch on the remote to reset to. Default: main.
  -h, --help         Show this help.

Safety:
  - Refuses to run outside a git worktree.
  - Refuses detached HEAD.
  - Refuses dirty worktrees unless --stash is passed.
  - Never force-pushes or deletes remote branches.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --confirm)
      CONFIRM=1
      shift
      ;;
    --stash)
      STASH=1
      shift
      ;;
    --remote)
      REMOTE="${2:-}"
      if [[ -z "$REMOTE" ]]; then
        echo "ERROR: --remote requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --branch)
      BRANCH="${2:-}"
      if [[ -z "$BRANCH" ]]; then
        echo "ERROR: --branch requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$STASH" -eq 1 && "$CONFIRM" -ne 1 ]]; then
  echo "ERROR: --stash requires --confirm" >&2
  exit 2
fi

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ERROR: not inside a git worktree" >&2
  exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

CURRENT_BRANCH="$(git branch --show-current)"
if [[ -z "$CURRENT_BRANCH" ]]; then
  echo "ERROR: detached HEAD. Check out a branch before resetting this worktree." >&2
  exit 2
fi

if ! git remote get-url "$REMOTE" >/dev/null 2>&1; then
  echo "ERROR: remote '$REMOTE' does not exist" >&2
  exit 2
fi

TARGET_REF="$REMOTE/$BRANCH"

echo "Fetching $REMOTE ..."
git fetch "$REMOTE" --prune >/dev/null

if ! git rev-parse --verify --quiet "$TARGET_REF^{commit}" >/dev/null; then
  echo "ERROR: target ref '$TARGET_REF' does not exist after fetch" >&2
  exit 2
fi

CURRENT_SHA="$(git rev-parse --short HEAD)"
TARGET_SHA="$(git rev-parse --short "$TARGET_REF")"
AHEAD_COUNT="$(git rev-list --count "$TARGET_REF"..HEAD)"
BEHIND_COUNT="$(git rev-list --count HEAD.."$TARGET_REF")"
DIRTY_STATUS="$(git status --porcelain)"

cat <<EOF

WORKTREE RESET PLAN
  repo:           $REPO_ROOT
  current branch: $CURRENT_BRANCH
  current HEAD:   $CURRENT_SHA
  target:         $TARGET_REF ($TARGET_SHA)
  ahead target:   $AHEAD_COUNT commit(s)
  behind target:  $BEHIND_COUNT commit(s)
  dirty:          $(if [[ -n "$DIRTY_STATUS" ]]; then echo yes; else echo no; fi)

EOF

if [[ -n "$DIRTY_STATUS" ]]; then
  echo "Dirty files:"
  git status --short
  echo
fi

if [[ "$CONFIRM" -ne 1 ]]; then
  cat <<EOF
Dry run only. To execute:
  $(realpath "$0") --confirm --remote $REMOTE --branch $BRANCH

If you intentionally want to preserve dirty local changes first:
  $(realpath "$0") --confirm --stash --remote $REMOTE --branch $BRANCH
EOF
  exit 0
fi

if [[ -n "$DIRTY_STATUS" ]]; then
  if [[ "$STASH" -eq 1 ]]; then
    STASH_MSG="pre reset-worktree-to-main: $CURRENT_BRANCH $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Stashing dirty worktree: $STASH_MSG"
    git stash push -u -m "$STASH_MSG" >/dev/null
  else
    echo "ERROR: worktree is dirty. Commit, stash, or rerun with --confirm --stash." >&2
    exit 2
  fi
fi

echo "Resetting $CURRENT_BRANCH to $TARGET_REF ..."
git reset --hard "$TARGET_REF"

cat <<EOF

DONE
  branch: $(git branch --show-current)
  head:   $(git rev-parse --short HEAD) $(git log -1 --pretty=%s)
EOF
