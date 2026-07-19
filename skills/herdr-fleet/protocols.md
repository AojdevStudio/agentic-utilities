# Fleet event-loop protocols

Consume only the watcher created for the current `$HERDR_WORKSPACE_ID`, `$HERDR_TAB_ID`, and `$PROJECT_KEY`. Before acting on an event, reject any pane whose live record does not match all three scope checks. Never consume another tab's fleet events.

Consume events one at a time with a persisted byte cursor. `--next` returns one complete, scope-validated NDJSON event without advancing the cursor; `--ack` advances atomically only after the control pane successfully handles it.

```bash
EVENT_SESSION_ID="fleet:${HERDR_WORKSPACE_ID}:${HERDR_TAB_ID}:${PROJECT_KEY}"
while true; do
  NEXT="$(bun <skill-directory>/scripts/consume-events.mjs --next \
    --events "$MONITOR_DIR/events.ndjson" \
    --cursor "$MONITOR_DIR/events.cursor" \
    --pid-file "$MONITOR_DIR/pid" \
    --session-id "$EVENT_SESSION_ID" \
    --wait-seconds 30)" || break
  status="$(printf '%s' "$NEXT" | bun -e '
const value = JSON.parse(await Bun.stdin.text());
process.stdout.write(value.status ?? "event");
')"
  [ "$status" = no_event ] && continue

  EVENT="$(printf '%s' "$NEXT" | bun -e '
process.stdout.write(JSON.stringify(JSON.parse(await Bun.stdin.text()).event));
')"
  NEXT_CURSOR="$(printf '%s' "$NEXT" | bun -e '
process.stdout.write(String(JSON.parse(await Bun.stdin.text()).nextCursor));
')"

  # Re-read any referenced pane and reject it unless workspace, tab, key, and owner metadata still match.
  # Read the transcript, perform the event action, and record evidence. Ack only after success.
  bun <skill-directory>/scripts/consume-events.mjs --ack "$NEXT_CURSOR" \
    --events "$MONITOR_DIR/events.ndjson" \
    --cursor "$MONITOR_DIR/events.cursor" \
    --session-id "$EVENT_SESSION_ID"
done
```

A crash before acknowledgment replays the unhandled event; a restart after acknowledgment resumes at the next byte. Partial trailing lines remain buffered on disk. If the watcher exits before another complete event, the consumer fails and the fleet becomes blocked rather than silently missing events.

For each accepted event, read the relevant transcript tail, act, and give the principal a proportionate update: a line for routine movement and a structured report for milestones.

## Consume review verdicts

Reviewers end each pull request with a `VERDICT:` block containing:

- `MERGE_READY`, `NEEDS_WORK`, or `BLOCKED`;
- the reviewed head SHA;
- required-gate status;
- bounded fix scope when applicable.

A verdict is fresh only when its SHA exactly equals the current pull-request head. Every head change invalidates every earlier verdict.

Act by verdict:

- **`MERGE_READY` plus green required checks:** apply the launch-time merge policy. Under `report-only`, report readiness and stop. Under `authorized-merge`, verify the repository, base branch, strategy, branch-deletion setting, current head SHA, and required checks all match the recorded authorization before merging from the control pane. A tool permission prompt still goes to the principal.
- **`NEEDS_WORK`:** dispatch the fix scope to the authoring implementer when healthy, otherwise to a free implementer with a self-contained brief. A push starts a fresh verdict cycle.
- **`BLOCKED`:** record the blocking dependency and owner. Escalate only decisions reserved for the principal.
- **Conflicting verdicts:** prioritize concrete, reproducible findings. Preserve complementary reviews and require all blocking findings to be resolved. A substantive `NEEDS_WORK` verdict supersedes a less substantive ready verdict at the same head.

A merge cycle is complete only when the policy decision, remote pull-request state, cleanup result, tracking update, and affected sibling pull requests are known.

## Handle every head change

Any content push, conflict resolution, metadata commit, or base sync invalidates the prior verdict.

- **Content change:** require a full review and a new verdict naming the new head SHA.
- **Mechanical base sync without source-file merges:** allow a short delta review, but require it to inspect the new head and issue a new verdict naming that SHA.
- **Base sync that auto-merges source files:** require a semantic delta review of each resolution and a new verdict naming the new head SHA.

Before merge, compare the verdict SHA with the live pull-request head again. A mismatch returns the pull request to review; it never inherits the old verdict.

## Resolve append-only shared-file conflicts

GitHub does not honor local `merge=union` drivers. A merge to the base branch can mark every open pull request carrying an append-only file as dirty, and server-side branch updates may refuse the conflict.

For each affected pull request, use its own worktree:

```bash
git fetch origin
git merge origin/<base-branch> --no-edit
# Resolve append-only files according to the repository's documented policy.
git push
```

Apply repository-required provenance and hooks. The resulting head requires a new delta verdict. Sequence merges to minimize repeated syncs, and defer control-authored base updates until the current pull-request wave lands.

## Triage red CI

Classify before rerunning:

1. Read the failing job log and identify the exact step or test.
2. Compare the failure with the pull-request diff.
3. Reproduce locally in the owning worktree or find the same failure on the base branch or unrelated runs.
4. For an environmental failure, record evidence and rerun once. On the second recurrence of the same class, create or assign a structural fix.
5. For a real regression, dispatch a `NEEDS_WORK` fix scope to the owner.

Triage is complete when the failure has an evidence-backed class, owner, and next action.

## Detect and salvage wedges

Suspect a wedge when context is near full, nested-agent timers run unusually long, or dispatches are ignored. Confirm with worktree modification times; a `working` status alone is not liveness evidence.

Before salvage, inspect the diff and run the smallest repository gate that checks the changed scope. Preserve valid work with an incremental commit in the worker's short-lived worktree, push it, and either finish the pull request with an explicit salvage note or hand it to a healthy implementer. Retire only a pane proven owned by this project, workspace, and tab. Start its replacement in the same scoped role with the full standing constraints and a self-contained brief.

## Recycle panes

Recycle a pane before a hard wedge when it shows excessive context growth, stale working directories, obsolete recaps, or unsubmitted input. Every replacement receives:

1. the full standing-constraint broadcast;
2. the repository-derived rules relevant to its role;
3. a self-contained assignment with current issue, branch, worktree, PR, and head details.

Update the watcher state by letting the old scoped pane disappear and the new scoped pane appear. Never broaden watcher scope to find a replacement.

## Handle watcher failure and shutdown

- Three consecutive scoped inventory failures stop the watcher. Treat this as `BLOCKED`, inspect its NDJSON error stream, restore socket health, and start one replacement watcher.
- Three context-read failures for one pane emit `pane_monitor_blocked`; inspect that pane directly without changing scope.
- On normal fleet shutdown, terminate the recorded watcher PID, wait for it, remove its PID file, and report the last consumed event.
- If the watcher PID is missing or no longer belongs to the recorded command, do not kill it; reconcile the monitor first.

## Communicate with the principal

- **Milestones:** report merges, tracking updates, salvages, and role changes in a short structured block.
- **Routine events:** acknowledge only when useful.
- **Unattended session:** use the configured notification mechanism for merges, blockers, and decisions.
- **Decisions:** use the available question tool for merge authorization, product direction, scope forks, spend, destructive actions, or policy exceptions. Put the recommended option first.
- **Unexpected actors or changes:** document evidence on the relevant tracking item and route the output through review with heightened scrutiny rather than automatically reverting or duplicating it.

The event is closed only when the action and its evidence are recorded in the control transcript.
