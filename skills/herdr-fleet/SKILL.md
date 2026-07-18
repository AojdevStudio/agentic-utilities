---
name: herdr-fleet
description: Orchestrate a multi-pane Herdr worker fleet of implementers and PR reviewers against a repository's issue and pull-request queues. Use when launching, rebuilding, or controlling a Herdr fleet. Requires HERDR_ENV=1.
---

# Herdr Fleet

Act as the fleet's control pane. The principal talks to this pane; the control pane dispatches work, consumes review verdicts, merges approved pull requests, and reports progress. Workers implement or review. Only the control pane operates Herdr or merges.

Choose the active branch:

- **Cold start** — no fleet exists or panes were lost: follow [launch-fleet.md](launch-fleet.md), then enter the live loop.
- **Live loop** — the fleet exists: follow [protocols.md](protocols.md).

## Standing rules

- Derive the control label from the current repository as `<project-key>-control-pane`; never copy a label from another project. [Launch Step 2](launch-fleet.md#step-2-claim-the-control-pane) defines the derivation.
- Reserve Herdr remote control for the control pane. Broadcast this boundary before assigning work.
- Merge only after a fresh review-queue verdict and green required checks. If policy blocks a merge, ask the principal directly; routing it through a worker is permission laundering.
- Delegate parallel work and retain only findings in the control context. Read worker transcripts with `herdr pane read`; dispatch with `herdr pane run`.
- Report events in proportion to their importance. Use the environment's notification mechanism for unattended merges, blockers, and decisions. Ask the principal only for genuine product, scope, cost, or irreversible decisions, with a recommendation first.
- Resolve reversible, PR-gated worker questions in the control pane. Verify every claim that the principal "approved" a choice.

## Default roster

Adapt commands to installed agents and the principal's preference; preserve the role split.

| Label suffix | Default command | Role |
|---|---|---|
| `claude-impl` | `claude --model sonnet` | implementer |
| `codex-impl` | `codex` | implementer |
| `pi-impl` | `pi` | implementer |
| `codex-review` | `codex` | pull-request reviewer |
| `claude-review` | `claude --model opus` | pull-request reviewer |

Prefix each label with the project key, for example `${PROJECT_KEY}-pi-impl`. Place the control pane left at full height, implementers in the middle column, and reviewers in the right column. Reviewers use the repository's review-queue skill when one exists; otherwise use the available code-review workflow.

## Operational gotchas

1. **Dialogs can swallow dispatches.** A message sent while a worker has a blocking permission or question dialog can become the dialog's answer. After dispatching to a blocked pane, read the transcript and confirm the message became a prompt. Audit any resulting approval claim.
2. **Hooks parse dispatch command text.** Repository hooks may regex-match the shell command containing `herdr pane run`, including words inside the worker message. When a hook trips, use its documented satisfaction path in separate commands rather than bypass environment variables.
3. **Capacity failures need bounded failover.** Retry a model-capacity error once. On a second failure, hand the task to another compatible worker and identify any orphaned review claim that must be superseded.
4. **Review claims race.** Two reviewers can claim after checking the same head. Duplicate reviews are acceptable, but only a review newer than the pull-request head is fresh. Explicitly override orphaned claims.
5. **Verdicts expire on head changes.** Content pushes require a fresh verdict. A mechanical base sync needs only a delta check when it auto-merges source files; dispatch that check whenever merge output names source files.
6. **Union merge drivers do not apply on GitHub.** A shared append-only file can make sibling pull requests dirty after every merge. Merge the base branch locally in each branch worktree, resolve with the repository's intended union behavior, and push.
7. **Context wedges masquerade as work.** A full context and a large nested-agent swarm can leave a pane reporting `working` while its worktree is frozen. Prevent this with incremental commits, bounded delegation, and compaction near 75%. Confirm liveness through worktree modification times. Salvage verified work, retire the pane, and start fresh.
8. **Long-lived sessions drift.** Recycle workers that accumulate very large contexts, use stale directories, repeat obsolete recaps, or leave dispatches unsubmitted. A fresh pane with a self-contained brief is safer.
9. **Runner starvation resembles test failure.** Classify failures before rerunning. A local pass, the same failure on unrelated runs, and high concurrent load point to infrastructure. Repeated failures of the same class warrant a structural CI fix.
10. **Workers exceed roles unless corrected.** Reviewers review; implementers implement; the control pane operates Herdr and merges. Watch queued intents and correct role drift before execution.
11. **Background deliverables may duplicate or disappear.** Request missing output explicitly. Act only on delivered content and deduplicate retries.
12. **Nested agents can finish files but fail to report.** If a worker waits on a silent child, inspect output modification times. When writes have stopped, tell the worker to verify disk state and finish inline.
13. **Long dispatches may paste without submitting.** Confirm every dispatch changes the pane to `working`. If a paste placeholder remains at the prompt, submit it with an empty `herdr pane run <id> ""`, then verify again.
14. **Repository policy is runtime input.** Before assigning work, derive lane labels, branch prefixes, provenance variables, hook requirements, issue-link rules, and worktree cleanup commands from the repository's checked-in instructions. Include only the applicable rules in each brief.
15. **Context footer formats vary.** Watch common forms such as `[79% ...]`, `90% context used`, and `9.3%/372k`. Re-arm compaction after usage drops below 60%; a permanent "already compacted" set disables protection for the rest of the session.
