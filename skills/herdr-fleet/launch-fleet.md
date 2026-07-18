# Launch the fleet

Build a worker fleet from a bare Herdr session and put it to work. Complete the steps in order.

## Step 1: Verify the environment

```bash
test "${HERDR_ENV:-}" = 1 || exit 1
herdr pane
printf '%s\n' "$HERDR_WORKSPACE_ID" "$HERDR_TAB_ID" "$HERDR_PANE_ID"
```

The installed `herdr pane` help is the syntax authority. Confirm each roster command exists with `command -v`; shell aliases are acceptable when worker panes load the same profile. Stop if the session or required agents are unavailable.

## Step 2: Claim the control pane

Derive a collision-resistant project key from the current repository. For multiword names, use each word's first letter. For a single word, use its first three characters.

```bash
project="$(basename "$(git rev-parse --show-toplevel 2>/dev/null || pwd)")"
PROJECT_KEY="$(printf '%s' "$project" | tr '[:upper:]' '[:lower:]' | awk -F'[-_ ]+' '{s=""; for (i=1;i<=NF;i++) s=s substr($i,1,1); if (length(s)<2) s=substr($1,1,3); print s}')"
CONTROL_LABEL="${PROJECT_KEY}-control-pane"
herdr pane rename "$HERDR_PANE_ID" "$CONTROL_LABEL"
herdr pane list --workspace "$HERDR_WORKSPACE_ID"
herdr pane layout --pane "$HERDR_PANE_ID"
```

Examples: `billing-api` becomes `ba-control-pane`; `widget` becomes `wid-control-pane`. Use `$PROJECT_KEY` and `$CONTROL_LABEL` everywhere. Scope all pane operations to the current workspace and tab.

## Step 3: Build the grid

Adapt to the roster size. For the default five workers in a wide pane:

1. Split the control pane right for the implementation column.
2. Split that pane right for the review column.
3. Split the implementation column down twice.
4. Split the review column down once.

Use `--no-focus`. Read every new `result.pane.pane_id` from command JSON; never predict pane IDs. Rename each pane immediately to `${PROJECT_KEY}-<role>`.

The step is complete when the control pane is full-height on the left, all five worker panes have recorded IDs and unique project-prefixed labels, and no unrelated workspace was changed.

## Step 4: Launch agents

Start each worker with only its normal interactive command:

```bash
herdr pane run <pane-id> "<agent-command>"
herdr wait agent-status <pane-id> --status idle --timeout 60000
```

Wait for every agent to reach `idle` before dispatching. Replace unavailable default commands with principal-approved installed agents while preserving the implementer/reviewer split.

## Step 5: Broadcast standing constraints

Send this before any task, substituting the derived label:

> STANDING CONSTRAINT: you are a worker pane. Remote control of this Herdr session is reserved for the control pane (`$CONTROL_LABEL`). Work only in your assigned role. Never run Herdr commands or merge pull requests. Never switch branches in the canonical checkout; use short-lived worktrees from the required remote base. Commit incrementally. Report results only in your own transcript.

For workers prone to nested delegation, add:

> Keep delegation bounded; do not spawn large parallel subagent swarms. The parent context is the scarce resource.

Read each pane transcript and verify the constraint arrived. Re-send it when the pane was blocked by a dialog or retained an unsubmitted paste.

## Step 6: Dispatch initial work

Before assigning anything, inspect the repository's checked-in agent instructions, contribution guide, pull-request template, issue labels, open pull requests, and worktree conventions.

- **Reviewers:** identify the repository review-queue workflow when present, include claim-mutex and fresh-head rules, name the peer reviewer to avoid duplicate claims, and list open pull requests.
- **Implementers:** split ready issues by repository-derived lane or ownership labels. Every brief includes existing-branch/PR detection, claim procedure, base branch, worktree and branch naming, verification commands, provenance/hook rules, and reporting requirements.
- **Thin queue:** assign open-PR advancement work or hold workers idle, then tell the principal the queue needs grooming. Do not invent work merely to keep panes busy.

The step is complete when every active worker has one self-contained assignment or an explicit idle state and no issue or pull request has conflicting owners.

## Step 7: Arm monitors

1. **Pane-status watcher:** poll `herdr pane list` about every 20 seconds and emit label/status changes. Treat `done`, `blocked`, and unexpected `idle` transitions as events.
2. **Context-threshold watcher:** about every five minutes, inspect each worker footer across the supported formats. At 75% usage, send `/compact`; at 8% or less until automatic compaction, record the risk. Re-arm after usage falls below 60%.
3. **CI waits:** create bounded, ad hoc waits per pull request that handle every terminal state, including cancellation and timeout.

The step is complete when status and context events are observable and every active pull request has a bounded check path.

## Step 8: Report and enter the loop

Report pane IDs, labels, roles, assignments, armed monitors, substitutions from the default roster, and queue shortages. Then follow [protocols.md](protocols.md).
