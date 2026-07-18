# Fleet event-loop protocols

The pane-status monitor drives the live loop. For each event, read the relevant transcript tail, act, and give the principal a proportionate update: a line for routine movement and a structured report for milestones.

## Consume review verdicts

Reviewers end each pull request with a `VERDICT:` block containing:

- `MERGE_READY`, `NEEDS_WORK`, or `BLOCKED`
- reviewed head SHA
- required-gate status
- bounded fix scope when applicable

Act by verdict:

- **`MERGE_READY` plus green required checks:** merge with the repository's approved strategy from the control pane. Verify the pull request's final state. Run the repository's worktree cleanup command when defined; otherwise prune only stale worktree metadata. Update linked tracking items, then identify open pull requests made dirty by the merge.
- **`NEEDS_WORK`:** dispatch the fix scope to the authoring implementer when healthy, otherwise to a free implementer with a self-contained brief. A push starts a fresh verdict cycle.
- **`BLOCKED`:** record the blocking dependency and owner. Escalate only decisions reserved for the principal.
- **Conflicting verdicts:** prioritize concrete, reproducible findings. Preserve complementary reviews and require all blocking findings to be resolved.

A merge cycle is complete only when the remote pull-request state, cleanup result, tracking update, and affected sibling pull requests are known.

## Handle head changes

- A content push invalidates earlier verdicts and requires full re-review at the new head.
- A mechanical base sync can retain the standing verdict unless it auto-merges source files. When merge output names source files, dispatch a focused delta check of the resolution before merging.

## Resolve append-only shared-file conflicts

GitHub does not honor local `merge=union` drivers. A merge to the base branch can mark every open pull request carrying an append-only file as dirty, and server-side branch updates may refuse the conflict.

For each affected pull request, use its own worktree:

```bash
git fetch origin
git merge origin/<base-branch> --no-edit
# Resolve append-only files according to the repository's documented policy.
git push
```

Apply repository-required provenance and hooks. Sequence merges to minimize repeated syncs, and defer control-authored base updates until the current pull-request wave lands.

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

Before salvage, inspect the diff and run the smallest repository gate that checks the changed scope. Preserve valid work with an incremental commit in the worker's short-lived worktree, push it, and either finish the pull request with an explicit salvage note or hand it to a healthy implementer. Retire only the pane and child processes created by this fleet, then start a fresh worker with the full standing constraints and a self-contained brief.

## Recycle panes

Recycle a pane before a hard wedge when it shows excessive context growth, stale working directories, obsolete recaps, or unsubmitted input. Every replacement receives:

1. the full standing-constraint broadcast;
2. the repository-derived rules relevant to its role;
3. a self-contained assignment with current issue, branch, worktree, PR, and head details.

## Communicate with the principal

- **Milestones:** report merges, tracking updates, salvages, and role changes in a short structured block.
- **Routine events:** acknowledge only when useful.
- **Unattended session:** use the configured notification mechanism for merges, blockers, and decisions.
- **Decisions:** use the available question tool for product direction, scope forks, spend, destructive actions, or policy exceptions. Put the recommended option first.
- **Unexpected actors or changes:** document evidence on the relevant tracking item and route the output through review with heightened scrutiny rather than automatically reverting or duplicating it.

The event is closed only when the action and its evidence are recorded in the control transcript.
