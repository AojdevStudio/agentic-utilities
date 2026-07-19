# Launch or rebuild the fleet

Build or reconcile a user-selected worker fleet inside the current Herdr tab. The current control pane is the only fixed pane and is not part of the worker count. Complete the steps in order.

## Step 1: Verify the environment and scope

```bash
test "${HERDR_ENV:-}" = 1 || exit 1
test -n "${HERDR_WORKSPACE_ID:-}" || exit 1
test -n "${HERDR_TAB_ID:-}" || exit 1
test -n "${HERDR_PANE_ID:-}" || exit 1
herdr pane
CURRENT_PANE_JSON="$(herdr pane current --current)"
printf '%s' "$CURRENT_PANE_JSON" | \
  EXPECTED_WORKSPACE="$HERDR_WORKSPACE_ID" EXPECTED_TAB="$HERDR_TAB_ID" EXPECTED_PANE="$HERDR_PANE_ID" \
  bun -e '
const payload = JSON.parse(await Bun.stdin.text());
const pane = payload?.result?.pane ?? payload?.pane ?? payload?.result ?? payload;
if (
  pane.workspace_id !== process.env.EXPECTED_WORKSPACE ||
  pane.tab_id !== process.env.EXPECTED_TAB ||
  pane.pane_id !== process.env.EXPECTED_PANE
) process.exit(1);
'
printf '%s\n' "$HERDR_WORKSPACE_ID" "$HERDR_TAB_ID" "$HERDR_PANE_ID"
```

The installed `herdr pane` output is the syntax authority. Stop before mutation if the session IDs are unavailable.

`herdr pane list` can filter by workspace but not tab. Every inventory must filter returned records by both `workspace_id` and `tab_id`; never treat a workspace-only result as the fleet.

## Step 2: Resolve the project identity

Derive the base key from the current repository. Multiword names use each word's first letter; a single word uses its first three characters.

```bash
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 1
project="$(basename "$REPO_ROOT")"
BASE_KEY="$(printf '%s' "$project" | tr '[:upper:]' '[:lower:]' | awk -F'[-_ ]+' '{s=""; for (i=1;i<=NF;i++) s=s substr($i,1,1); if(length(s)<2)s=substr($1,1,3); print s}')"
ALL_PANES_JSON="$(herdr pane list --workspace "$HERDR_WORKSPACE_ID")"
TAB_PANES_JSON="$(printf '%s' "$ALL_PANES_JSON" | TAB_ID="$HERDR_TAB_ID" WORKSPACE_ID="$HERDR_WORKSPACE_ID" bun -e '
const payload = JSON.parse(await Bun.stdin.text());
const panes = payload?.result?.panes ?? payload?.panes ?? payload?.result ?? payload;
const scoped = (Array.isArray(panes) ? panes : []).filter(
  (pane) => pane.workspace_id === process.env.WORKSPACE_ID && pane.tab_id === process.env.TAB_ID,
);
process.stdout.write(JSON.stringify(scoped));
')"
KEY_RESULT="$(printf '%s' "$TAB_PANES_JSON" | bun <skill-directory>/scripts/resolve-project-key.mjs \
  --base-key "$BASE_KEY" --repo-root "$REPO_ROOT" --json)"
PROJECT_KEY="$(printf '%s' "$KEY_RESULT" | bun -e 'process.stdout.write(JSON.parse(await Bun.stdin.text()).projectKey)')"
FLEET_OWNER_TOKEN="$(printf '%s' "$KEY_RESULT" | bun -e 'process.stdout.write(JSON.parse(await Bun.stdin.text()).ownerToken)')"
CONTROL_LABEL="${PROJECT_KEY}-control-pane"
```

The resolver prefers an established ownership-proven key. It compares metadata keys and legacy key/role labels by complete boundaries, so `ba-1234-pi-impl` occupies `ba-1234`, not `ba`. If the exact base key is foreign-occupied, it derives a deterministic suffix, rechecks that complete key, and lengthens the suffix until free. Multiple owned keys stop the launch for reconciliation.

## Step 3: Collect and confirm the roster

Read the human-facing [launcher menu](README.md#launcher-menu), then use `AskUserQuestion` before creating any worker pane.

### Rebuild intake

First inspect current-tab panes for ownership metadata:

- `tokens.fleet_owner` equals `$FLEET_OWNER_TOKEN`;
- `tokens.fleet_key` equals `$PROJECT_KEY`;
- `tokens.fleet_kind` is `worker`;
- repository cwd is owned;
- `fleet_label`, `fleet_role`, `fleet_command`, `fleet_placement`, and optional `fleet_assignment` are present;
- the exact stored command and placement reconstruct the prior roster, and `pane process-info` is consistent with the stored launcher.

When every surviving worker has complete, consistent metadata, reconstruct the prior roster and use `AskUserQuestion` with **Reuse detected roster**, **Edit roster**, and **Cancel**. Include the full pane-map preview. Reuse only after confirmation.

If any worker is ambiguous, metadata is incomplete, command evidence differs, or no owned roster exists, collect the roster again.

### New or edited intake

Use a free-form `AskUserQuestion` asking for one worker per line:

```text
label | launch command | role | optional assignment/lane | desired placement
```

Requirements:

- accept any number of workers, including zero;
- require a unique non-empty label and launch command per worker;
- accept `implementer`, `reviewer`, or any user-specified role;
- preserve an optional assignment or lane constraint;
- accept `pi`, `codex`, every documented Claudex/Claude launcher, and arbitrary user commands;
- treat commands and assignments as data: never interpolate credentials or secrets into pane labels or metadata.

Validate the roster, derive the exact split/placement plan, and render a preview containing the control pane plus every worker's label, command, role, assignment/lane, and placement. Use a second `AskUserQuestion` with **Confirm**, **Edit**, and **Cancel**. No pane mutation occurs before **Confirm**.

The confirmed roster is the source of truth for all later role, launch, and assignment decisions. There is no default worker count or default worker roster.

## Step 4: Establish merge policy

Use `AskUserQuestion` to establish merge policy. Default to `report-only` and record the answer in the control transcript.

| Policy | Behavior |
|---|---|
| `report-only` | Stop at a fresh `MERGE_READY` verdict and report it. This is the default. |
| `authorized-merge` | Merge only on explicitly allowed base branches, with the explicitly selected strategy, after a fresh verdict and green required checks. |

Authorization must name the repository, allowed base branches, merge strategy, and branch-deletion policy. It does not bypass tool permissions; approval prompts go directly to the principal.

## Step 5: Reconcile confirmed workers

Before renaming, splitting, or launching, compare the confirmed roster with exact ownership metadata from `$TAB_PANES_JSON`. Inspect each candidate with `herdr pane get`, `herdr pane process-info`, and a recent transcript read.

Classify each confirmed entry:

- **Healthy owned match:** identity metadata and command evidence match the confirmed entry, and status is `idle`, `working`, `blocked`, or `done`. Reuse it.
- **Stale owned match:** identity matches, but the process exited, returned to a shell, or is otherwise proven abandoned. Retire only this pane.
- **User-confirmed legacy match:** repository cwd and a complete legacy key/role label match, and the user explicitly maps it to a confirmed entry after command inspection. Reuse it, then stamp current metadata in Step 7.
- **Foreign or ambiguous pane:** ownership evidence is missing or mismatched. Leave it untouched and do not create a same-label replacement without another user decision.
- **Duplicate healthy label:** stop and ask which pane to keep.
- **Missing entry:** record it for Step 6.

Also identify owned workers absent from the confirmed roster. Ask before retiring them; roster confirmation alone is not process-kill consent.

Close a stale pane only after recording evidence and its pane ID, then refresh the workspace inventory and re-filter the current tab:

```bash
herdr pane close <proven-stale-owned-pane-id>
```

If another healthy `${CONTROL_LABEL}` exists outside the current pane, stop and direct the principal to it. Replace it only when proven stale or explicitly authorized. Then claim and verify the current control pane:

```bash
herdr pane rename "$HERDR_PANE_ID" "$CONTROL_LABEL"
herdr pane layout --pane "$HERDR_PANE_ID"
```

## Step 6: Create only missing confirmed workers

Follow the confirmed pane-map placements. Use explicit source pane IDs from the current-tab inventory for every split. Before each split, verify the source still belongs to `$HERDR_WORKSPACE_ID` and `$HERDR_TAB_ID`. Always use `--no-focus`; read `result.pane.pane_id` from JSON; verify the returned workspace and tab; never predict IDs.

Create exactly one pane for each missing confirmed roster entry. Any number of workers is valid. A partial surviving fleet becomes reused-plus-missing, never duplicated.

## Step 7: Launch new workers and stamp all confirmed workers

Start each new worker with its confirmed command, wait for its interactive agent when applicable, and verify the command through `pane process-info`. Reused workers keep their sessions. Before watcher startup, stamp every confirmed new, reused, or user-confirmed legacy worker with current metadata.

```bash
herdr pane rename <pane-id> "${PROJECT_KEY}-<confirmed-worker-label>"
herdr pane run <pane-id> "<confirmed-launch-command>"
herdr wait agent-status <pane-id> --status idle --timeout 60000
```

Record reconstructable ownership metadata on every confirmed worker. Omit `fleet_assignment` only when the user left it empty.

```bash
herdr pane report-metadata <pane-id> \
  --source user:herdr-fleet \
  --token "fleet_owner=$FLEET_OWNER_TOKEN" \
  --token "fleet_key=$PROJECT_KEY" \
  --token "fleet_kind=worker" \
  --token "fleet_label=<confirmed-worker-label>" \
  --token "fleet_role=<confirmed-role>" \
  --token "fleet_assignment=<confirmed-assignment-or-lane>" \
  --token "fleet_command=<confirmed-launch-command>" \
  --token "fleet_placement=<confirmed-placement>"
```

Herdr metadata values are bounded. Store commands only when they contain no credential and fit without truncation. If any roster value cannot be stored exactly, mark that pane non-reusable and require intake on the next rebuild rather than claiming proof that does not exist.

## Step 8: Broadcast standing constraints

Send this to every new or reused worker before assignment:

> STANDING CONSTRAINT: you are a worker pane. Remote control of this Herdr session is reserved for the control pane (`$CONTROL_LABEL`). Follow your confirmed role and assignment. Never run Herdr commands or merge pull requests. Never switch branches in the canonical checkout; use short-lived worktrees from the required remote base. Commit incrementally. Report results only in your own transcript.

For workers prone to nested delegation, add bounded-delegation guidance. Read each transcript and verify delivery; re-send after blocked dialogs or unsubmitted pastes.

## Step 9: Dispatch confirmed assignments

Inspect repository instructions, contribution guidance, pull-request templates, issue labels, open pull requests, and worktree conventions first.

- **Implementers:** include claim procedure, branch/worktree rules, gates, hooks, and the confirmed assignment/lane.
- **Reviewers:** include the review workflow, claim mutex, peer labels, fresh-head verdict requirement, and confirmed queue scope.
- **Other roles:** dispatch only the user-confirmed responsibility and boundaries.
- **Unassigned workers:** hold idle or ask; do not invent work merely to occupy panes.

## Step 10: Arm the scoped watcher

A confirmed zero-worker roster needs no watcher. Otherwise, start or reuse exactly one project-scoped watcher:

```bash
MONITOR_DIR="${TMPDIR:-/tmp}/herdr-fleet-${HERDR_WORKSPACE_ID//:/_}-${HERDR_TAB_ID//:/_}-${PROJECT_KEY}"
mkdir -p "$MONITOR_DIR"
REUSE_WATCHER=no
if [ -f "$MONITOR_DIR/pid" ]; then
  existing_pid="$(cat "$MONITOR_DIR/pid")"
  existing_command="$(ps -p "$existing_pid" -o command= 2>/dev/null || true)"
  case "$existing_command" in
    *"watch-fleet.mjs --project-key $PROJECT_KEY --owner-token $FLEET_OWNER_TOKEN"*)
      kill -0 "$existing_pid" 2>/dev/null && REUSE_WATCHER=yes
      ;;
  esac
fi

if [ "$REUSE_WATCHER" = yes ]; then
  FLEET_WATCHER_PID="$existing_pid"
else
  rm -f "$MONITOR_DIR/pid"
  mkdir "$MONITOR_DIR/start.lock" || exit 1
  trap 'rmdir "$MONITOR_DIR/start.lock" 2>/dev/null || true' EXIT INT TERM
  bun <skill-directory>/scripts/watch-fleet.mjs \
    --project-key "$PROJECT_KEY" --owner-token "$FLEET_OWNER_TOKEN" \
    >"$MONITOR_DIR/events.ndjson" 2>"$MONITOR_DIR/errors.ndjson" &
  FLEET_WATCHER_PID=$!
  printf '%s\n' "$FLEET_WATCHER_PID" >"$MONITOR_DIR/pid.tmp"
  mv "$MONITOR_DIR/pid.tmp" "$MONITOR_DIR/pid"
  rmdir "$MONITOR_DIR/start.lock"
  trap - EXIT INT TERM
  sleep 1
  kill -0 "$FLEET_WATCHER_PID" 2>/dev/null || exit 1
fi
```

The watcher filters by workspace, tab, exact project identity, and ownership token; requires a worker during bounded startup; tracks status changes; parses only current context footers; retries compaction protection; and stops after bounded failures. Treat watcher exit as a blocker. Consume only its NDJSON, never global Herdr events.

For shutdown:

```bash
kill "$(cat "$MONITOR_DIR/pid")"
wait "$(cat "$MONITOR_DIR/pid")" 2>/dev/null || true
rm -f "$MONITOR_DIR/pid"
```

Create bounded CI waits per pull request covering success, failure, cancellation, and timeout.

## Step 11: Report and enter the loop

Report merge policy, workspace/tab IDs, project key, confirmed roster, pane IDs, reused/created/retired workers, assignments, watcher PID, and any blockers. Then follow [protocols.md](protocols.md).
