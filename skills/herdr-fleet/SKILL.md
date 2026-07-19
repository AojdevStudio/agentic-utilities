---
name: herdr-fleet
description: This skill should be used when a project requires Herdr fleet launch, worker-pane startup, surviving-pane rebuilds, or control of a project-scoped issue and pull-request queue. Requires HERDR_ENV=1.
---

# Herdr Fleet

Act as the fleet's control pane. The principal talks to this pane; the control pane dispatches work, consumes review verdicts, and reports progress. Workers implement or review. Only the control pane operates Herdr. Merge authority is established explicitly during launch and defaults to report-only.

Choose the active branch:

- **Cold start** — no fleet exists or panes were lost: follow [launch-fleet.md](launch-fleet.md), then enter the live loop.
- **Live loop** — the fleet exists: follow [protocols.md](protocols.md).

## Standing rules

- Derive a collision-checked control label from the current repository as `<project-key>-control-pane`; never copy a label from another project. [Launch Step 2](launch-fleet.md#step-2-resolve-the-project-identity) defines the derivation.
- Scope every inventory, watcher, split, discovery, and event action to both the injected workspace and tab IDs.
- Reconcile surviving project-owned panes before creating anything. Reuse healthy panes, retire only proven stale owned panes, and create only missing roles.
- Reserve Herdr remote control for the control pane. Broadcast this boundary before assigning work.
- Default to report-only. Merge only when explicit launch-time authorization covers the repository, base branch, and strategy, and a fresh verdict names the current head with green required checks. If policy or tooling requires approval, ask the principal directly; routing it through a worker is permission laundering.
- Delegate parallel work and retain only findings in the control context. Read worker transcripts with `herdr pane read`; dispatch with `herdr pane run`.
- Report events in proportion to their importance. Use the environment's notification mechanism for unattended merges, blockers, and decisions. Ask the principal only for genuine product, scope, cost, or irreversible decisions, with a recommendation first.
- Resolve reversible, PR-gated worker questions in the control pane. Verify every claim that the principal "approved" a choice.

## User-selected roster

The current control pane is the only fixed pane. Before creating workers, follow the `AskUserQuestion` intake and confirmation in [launch-fleet.md](launch-fleet.md#step-3-collect-and-confirm-the-roster). Accept any worker count, labels, commands, roles, assignments, and confirmed pane map. Rebuilds reuse only ownership-proven roster metadata.

Read the human [launcher menu](README.md#launcher-menu) before presenting command choices. Claudex, native Claude Code, Pi, Codex, and arbitrary user-provided commands are options, never roster defaults.

## Operational gotchas

1. **Dialogs can swallow dispatches.** A message sent while a worker has a blocking permission or question dialog can become the dialog's answer. After dispatching to a blocked pane, read the transcript and confirm the message became a prompt. Audit any resulting approval claim.
2. **Hooks parse dispatch command text.** Repository hooks may regex-match the shell command containing `herdr pane run`, including words inside the worker message. When a hook trips, use its documented satisfaction path in separate commands rather than bypass environment variables.
3. **Capacity failures need bounded failover.** Retry a model-capacity error once. On a second failure, hand the task to another compatible worker and identify any orphaned review claim that must be superseded.
4. **Review claims race.** Two reviewers can claim after checking the same head. Duplicate reviews are acceptable, but only a review newer than the pull-request head is fresh. Explicitly override orphaned claims.
5. **Verdicts expire on every head change.** Content pushes require full review. Mechanical base syncs may use a focused delta review, but that review must issue a new verdict naming the post-sync SHA; source-file resolutions receive semantic scrutiny.
6. **Union merge drivers do not apply on GitHub.** A shared append-only file can make sibling pull requests dirty after every merge. Merge the base branch locally in each branch worktree, resolve with the repository's intended union behavior, and push.
7. **Context wedges masquerade as work.** A full context and a large nested-agent swarm can leave a pane reporting `working` while its worktree is frozen. Prevent this with incremental commits, bounded delegation, and compaction near 75%. Confirm liveness through worktree modification times. Salvage verified work, retire the pane, and start fresh.
8. **Long-lived sessions drift.** Recycle workers that accumulate very large contexts, use stale directories, repeat obsolete recaps, or leave dispatches unsubmitted. A fresh pane with a self-contained brief is safer.
9. **Runner starvation resembles test failure.** Classify failures before rerunning. A local pass, the same failure on unrelated runs, and high concurrent load point to infrastructure. Repeated failures of the same class warrant a structural CI fix.
10. **Workers exceed roles unless corrected.** Reviewers review; implementers implement; the control pane operates Herdr and applies the recorded merge policy. Watch queued intents and correct role drift before execution.
11. **Background deliverables may duplicate or disappear.** Request missing output explicitly. Act only on delivered content and deduplicate retries.
12. **Nested agents can finish files but fail to report.** If a worker waits on a silent child, inspect output modification times. When writes have stopped, tell the worker to verify disk state and finish inline.
13. **Long dispatches may paste without submitting.** Confirm every dispatch changes the pane to `working`. If a paste placeholder remains at the prompt, submit it with an empty `herdr pane run <id> ""`, then verify again.
14. **Repository policy is runtime input.** Before assigning work, derive lane labels, branch prefixes, provenance variables, hook requirements, issue-link rules, and worktree cleanup commands from the repository's checked-in instructions. Include only the applicable rules in each brief.
15. **Context footer formats vary.** The scoped watcher recognizes forms such as `[79% ...]`, `90% context used`, and `9.3%/372k`. It re-arms compaction after usage drops below 60%; a permanent "already compacted" set would disable protection for the rest of the session.
