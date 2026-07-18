# Launch or rebuild the fleet

Build or reconcile a worker fleet inside the current Herdr tab. Complete the steps in order. A rebuild reuses a healthy partial fleet; it never creates a second copy.

## Step 1: Verify the environment and scope

```bash
test "${HERDR_ENV:-}" = 1 || exit 1
test -n "${HERDR_WORKSPACE_ID:-}" || exit 1
test -n "${HERDR_TAB_ID:-}" || exit 1
test -n "${HERDR_PANE_ID:-}" || exit 1
herdr pane
printf '%s\n' "$HERDR_WORKSPACE_ID" "$HERDR_TAB_ID" "$HERDR_PANE_ID"
```

The installed `herdr pane` output is the syntax authority. Confirm each roster command exists with `command -v`. Stop before mutation if the session IDs or required agents are unavailable.

`herdr pane list` can filter by workspace but not tab. Every inventory in this workflow must therefore filter returned pane records by both `workspace_id` and `tab_id`. Never treat a workspace-only result as the fleet.

## Step 2: Derive a unique project key

Derive the base key from the current repository. Multiword names use each word's first letter; a single word uses its first three characters.

```bash
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
project="$(basename "$REPO_ROOT")"
BASE_KEY="$(printf '%s' "$project" | tr '[:upper:]' '[:lower:]' | awk -F'[-_ ]+' '{s=""; for (i=1;i<=NF;i++) s=s substr($i,1,1); if (length(s)<2) s=substr($1,1,3); print s}')"
ALL_PANES_JSON="$(herdr pane list --workspace "$HERDR_WORKSPACE_ID")"
TAB_PANES_JSON="$(printf '%s' "$ALL_PANES_JSON" | TAB_ID="$HERDR_TAB_ID" WORKSPACE_ID="$HERDR_WORKSPACE_ID" bun -e '
const payload = JSON.parse(await Bun.stdin.text());
const panes = payload?.result?.panes ?? payload?.panes ?? payload?.result ?? payload;
const scoped = (Array.isArray(panes) ? panes : []).filter(
  (pane) => pane.workspace_id === process.env.WORKSPACE_ID && pane.tab_id === process.env.TAB_ID,
);
process.stdout.write(JSON.stringify(scoped));
')"
```

Detect an existing foreign use of the base key in this tab. Ownership requires both the key prefix and a pane `cwd`/`foreground_cwd` inside `$REPO_ROOT`; a matching label alone is not ownership.

```bash
KEY_COLLISION="$(printf '%s' "$TAB_PANES_JSON" | BASE_KEY="$BASE_KEY" REPO_ROOT="$REPO_ROOT" bun -e '
import path from "node:path";
const panes = JSON.parse(await Bun.stdin.text());
const root = path.resolve(process.env.REPO_ROOT);
const ownsPath = (value) => value && (path.resolve(value) === root || path.resolve(value).startsWith(`${root}${path.sep}`));
const collision = panes.some((pane) => {
  if (typeof pane.label !== "string" || !pane.label.startsWith(`${process.env.BASE_KEY}-`)) return false;
  return !ownsPath(pane.foreground_cwd ?? pane.cwd);
});
process.stdout.write(collision ? "yes" : "no");
')"
PROJECT_KEY="$BASE_KEY"
if [ "$KEY_COLLISION" = yes ]; then
  suffix="$(printf '%s' "$REPO_ROOT" | git hash-object --stdin | cut -c1-4)"
  PROJECT_KEY="${BASE_KEY}-${suffix}"
fi
CONTROL_LABEL="${PROJECT_KEY}-control-pane"
```

Examples: `billing-api` normally becomes `ba-control-pane`; `widget` becomes `wid-control-pane`. A colliding key gains a deterministic repository-derived suffix. Re-run the scoped inventory with the resolved prefix before continuing.

## Step 3: Establish merge policy

Ask the principal which merge policy this fleet has. Default to report-only and record the answer in the control transcript.

| Policy | Behavior |
|---|---|
| `report-only` | Stop at a fresh `MERGE_READY` verdict and report it. This is the default. |
| `authorized-merge` | Merge only on explicitly allowed base branches, with the explicitly selected strategy, after a fresh verdict and green required checks. |

Authorization must name:

1. allowed repository and base branch or branches;
2. merge strategy;
3. whether branch deletion is allowed.

Authorization does not bypass tool or repository permissions. If a merge command still requires approval, ask the principal directly. Never route it through a worker.

## Step 4: Reconcile the current-tab fleet

Before renaming, splitting, or launching anything, inventory the resolved `${PROJECT_KEY}-` labels from `$TAB_PANES_JSON` and inspect each candidate with `herdr pane get`, `herdr pane process-info`, and a recent transcript read.

Classify each expected role:

- **Healthy owned pane:** project-prefixed label, current workspace and tab, repository cwd, and an expected agent in `idle`, `working`, `blocked`, or `done`. Reuse it.
- **Stale owned pane:** the same ownership evidence, but the process exited, returned to a plain shell, or is otherwise proven abandoned. Retire only this pane.
- **Foreign or ambiguous pane:** ownership evidence is missing or points outside the repository. Leave it untouched.
- **Duplicate healthy role:** stop and ask which pane to keep. Do not close or create another.
- **Missing role:** record it for Step 5.

Close a stale pane only after recording its ownership evidence and pane ID, then refresh and re-filter the current-tab inventory before any other mutation:

```bash
herdr pane close <proven-stale-owned-pane-id>
```

If another healthy `${CONTROL_LABEL}` exists and is not the current pane, stop and direct the principal to that control pane. Replace it only when it is proven stale or the principal explicitly authorizes replacement. Then claim the current pane:

```bash
herdr pane rename "$HERDR_PANE_ID" "$CONTROL_LABEL"
herdr pane layout --pane "$HERDR_PANE_ID"
```

Re-read the pane and confirm its `workspace_id` and `tab_id` match the injected IDs. Reconciliation is complete when every surviving pane has one role, every missing role is known, and no foreign pane was mutated.

## Step 5: Create only missing roles

For the default five-worker roster, preserve the intended layout: control pane left at full height, implementers in the middle column, reviewers in the right column.

Use explicit source pane IDs from the current-tab inventory for every split. Before each split, verify the source pane still belongs to `$HERDR_WORKSPACE_ID` and `$HERDR_TAB_ID`. Always use `--no-focus`, read `result.pane.pane_id` from JSON, verify the returned pane's workspace and tab, then rename it `${PROJECT_KEY}-<role>`. Never predict pane IDs.

Create only roles marked missing in Step 4. A partial surviving fleet must remain partial-plus-new, never duplicated.

## Step 6: Launch only new agents

Start each newly created worker with only its normal interactive command:

```bash
herdr pane run <pane-id> "<agent-command>"
herdr wait agent-status <pane-id> --status idle --timeout 60000
```

Wait for every new agent to reach `idle`. Reused healthy panes keep their sessions. Replace unavailable default commands only with principal-approved installed agents while preserving the implementer/reviewer split.

## Step 7: Broadcast standing constraints

Send this to every new or reused worker before assigning work:

> STANDING CONSTRAINT: you are a worker pane. Remote control of this Herdr session is reserved for the control pane (`$CONTROL_LABEL`). Work only in your assigned role. Never run Herdr commands or merge pull requests. Never switch branches in the canonical checkout; use short-lived worktrees from the required remote base. Commit incrementally. Report results only in your own transcript.

For workers prone to nested delegation, add:

> Keep delegation bounded; do not spawn large parallel subagent swarms. The parent context is the scarce resource.

Read each pane transcript and verify delivery. Re-send when a pane was blocked by a dialog or retained an unsubmitted paste.

## Step 8: Dispatch initial work

Inspect the repository's checked-in agent instructions, contribution guide, pull-request template, issue labels, open pull requests, and worktree conventions first.

- **Reviewers:** include the repository review workflow, claim mutex, fresh-head verdict requirement, peer reviewer label, and current open pull requests.
- **Implementers:** split ready issues by repository-derived ownership. Every brief includes existing-branch/PR detection, claim procedure, base branch, worktree and branch naming, verification commands, provenance/hook rules, and reporting requirements.
- **Thin queue:** assign open-PR advancement work or hold workers idle, then report the shortage. Do not invent work merely to occupy panes.

The step is complete when every active worker has one self-contained assignment or an explicit idle state and no item has conflicting owners.

## Step 9: Arm the scoped watcher

Resolve `scripts/watch-fleet.mjs` relative to this skill directory. Start exactly one persistent watcher and record its PID and output paths:

```bash
MONITOR_DIR="${TMPDIR:-/tmp}/herdr-fleet-${HERDR_WORKSPACE_ID//:/_}-${HERDR_TAB_ID//:/_}"
mkdir -p "$MONITOR_DIR"
bun <skill-directory>/scripts/watch-fleet.mjs --project-key "$PROJECT_KEY" \
  >"$MONITOR_DIR/events.ndjson" 2>"$MONITOR_DIR/errors.ndjson" &
FLEET_WATCHER_PID=$!
printf '%s\n' "$FLEET_WATCHER_PID" >"$MONITOR_DIR/pid"
```

The watcher:

- inventories with `pane list --workspace`, then rejects records outside the current `workspace_id`, `tab_id`, and project-label prefix;
- polls status every 20 seconds and emits only state changes as NDJSON;
- polls visible context every five minutes, recognizes the supported footer formats, requests `/compact` at 75%, defers blocked dialogs, and re-arms after usage falls to 60%;
- exits nonzero after three consecutive inventory failures and emits per-pane monitor failures after three context-read failures.

Treat watcher exit as a fleet blocker; restore socket health and restart one watcher rather than falling back to unscoped polling. Consume only this watcher's NDJSON. Do not consume global Herdr events. On `compaction_requested`, read that pane and verify `/compact` was accepted or visibly queued; apply Gotcha 13 from [SKILL.md](SKILL.md) when it remained as an unsubmitted paste.

For shutdown:

```bash
kill "$(cat "$MONITOR_DIR/pid")"
wait "$(cat "$MONITOR_DIR/pid")" 2>/dev/null || true
rm -f "$MONITOR_DIR/pid"
```

Create bounded CI waits per pull request that handle success, failure, cancellation, and timeout.

## Step 10: Report and enter the loop

Report merge policy, workspace/tab IDs, project key, pane IDs, reused/created/retired roles, assignments, watcher PID, substitutions, and queue shortages. Then follow [protocols.md](protocols.md).
