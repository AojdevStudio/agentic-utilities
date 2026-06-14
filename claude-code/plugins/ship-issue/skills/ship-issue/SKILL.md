---
name: ship-issue
description: Execute GitHub issues one at a time as vertical slices — fetch, branch, TDD, verify, PR, then the next. Use when the user says "ship issue", "resolve #N", "work through these issues", or after `to-issues` to execute the queue.
---

# Ship Issue

Execute GitHub issues one at a time. One issue, one branch, one PR per issue — then, if more were queued, start the next.

The `to-issues` skill divided the work into vertical slices on purpose. This skill exists to keep you from re-horizontalizing them during execution. Sequential ≠ parallel ≠ batched. Scope discipline is the only thing that makes those slices worth anything.

## Rules

- **One at a time, sequential.** If the user lists multiple issues, work through them in order — ship one completely, then start the next. Never open two PRs that touch overlapping code. Never squash multiple issues into one PR.
- **The acceptance criteria are the spec.** Not "what I think they meant." If a criterion is ambiguous or a decision is missing, STOP and route back to `grill-me` or `request-refactor-plan`. Do not guess.
- **The issue body is immutable.** If mid-flight you discover the scope is wrong, stop and file a follow-up issue. Do not silently expand the PR.
- **No scope creep.** Refactors and cleanups you notice go in a NEW issue, not this PR. Fowler: "make each refactoring step as small as possible."
- **Use `tdd` for the inner loop.** Red → green → refactor, one behavior at a time. Horizontal slicing (all tests, then all code) is forbidden — see the `tdd` skill for why.
- **Update the project board.** If `docs/agents/workflow.md` names a Project board, move the issue to `In Progress` when starting and to the repo's terminal status only after merge or explicit handoff.
- **Self-review before PR.** Before opening or updating the PR, review the diff yourself, fix obvious issues, and include the self-review result in the PR handoff.
- **"Done" requires functional evidence.** Tests passing is necessary, not sufficient. Run the feature end-to-end. Hit the endpoint. Render the page. Capture the output. If you cannot execute a functional test, say so — do not claim done.
- **PR opened is not shipped.** Do not say "shipped" unless the PR is merged or the user explicitly asked for PR-open-only delivery. If the PR is open and still waiting on CI/review/merge, report it as `PR opened; closeout pending`.

## Completion states

An issue is complete only when one of these terminal states is true:

- **Merged:** the PR is merged, the branch cleanup result is known, and the issue/PR closeout is reported.
- **Explicit PR-open handoff:** the user asked only for a PR, or denied auto-merge. Report the PR URL, pending checks/reviews, and the exact remaining command or automation.
- **Blocked:** a real external blocker prevents progress. Add the blocker to the issue or PR when useful, then report the blocker and the last verified state.

If the PR is open but not terminal, continue the closeout loop in the same turn when possible. If waiting is required, set up a concrete follow-up mechanism:

- Invoke `babysit-pr` to own PR lifecycle: CI polling, failed-check repair, merge readiness, and merge/alert terminal state.
- Invoke `greploop` whenever Greptile/review automation/reviewers leave actionable comments or the PR has not reached the required 5/5 review confidence.
- If waiting is required, use the environment's available follow-up mechanism to resume the same `babysit-pr`/`greploop` workflow. The follow-up prompt must include the PR number, required merge decision, pending checks/reviews, and the next command to run.
- If follow-up cannot be created, say `NOT COMPLETE: PR closeout still pending` and include the manual next step. Do not proceed to the next queued issue.

## Agent delegation

The main agent is the controller. It should preserve its context for issue scope, PR state, merge decision, and final reporting. For substantial CI or review work, delegate bounded work to worker agents instead of pulling all logs, comments, and file context into the main thread.

- Use worker agents for independent domains: separate failed CI jobs, independent review-comment clusters, or disjoint subsystems.
- Give each worker only the PR number, branch, exact failure/comment IDs, relevant commands, and owned file/subsystem scope.
- Do not run parallel write agents against overlapping files. If ownership overlaps, serialize the fixes or assign one worker to the whole cluster.
- Require each worker to return a compact packet: changed files, commits pushed, validation commands/results, unresolved blockers, and whether `greploop`/CI should continue.
- The controller integrates results, checks for conflicts, keeps `babysit-pr` moving, and decides whether the PR has reached a terminal state.

## Process

0. **Sync with main.** Before fetching or branching, make sure the current worktree is pulled up to date with `main` so issue work starts from the latest merged code.
1. **Fetch the issue.** `gh issue view <number> --comments`. Read it fully.
2. **Verify blockers are closed.** If the issue's "Blocked by" field lists open issues, stop. Ship those first.
3. **Move project status to In Progress.** If a project board is configured, update the item before editing code.
4. **Check for ambiguity.** If any acceptance criterion is unclear, or if required decisions are missing, STOP and tell the user to invoke `grill-me` or `request-refactor-plan`. Do not proceed.
5. **Create a branch.** Name it from the issue: e.g. `issue-<number>-<short-slug>`.
6. **TDD the acceptance criteria.** Invoke the `tdd` skill. Drive ONE criterion at a time through RED → GREEN → REFACTOR. Check each criterion off in the issue as its test passes.
7. **Stay inside the slice.** Only refactor code that directly supports this issue. Anything else → file a new issue and leave it for later.
8. **Functional verification.** Before opening the PR, execute the feature end-to-end and capture evidence (curl output, screenshot, CLI run). Paste the evidence into the PR body.
9. **Self-review the diff.** Review changed files against the issue, acceptance criteria, tests, and project docs. Fix actionable findings before opening/updating the PR.
10. **Submit the PR.** Invoke the `gitworkflow` skill. Title: the issue title. Body: `Closes #<number>`, the acceptance-criteria checklist (all checked), the functional evidence from step 8, and the self-review result from step 9.
11. **Babysit the PR.** Invoke the `babysit-pr` skill and monitor checks/review state until the PR reaches a merge decision.
12. **Run greploop when review is not done.** If Greptile is below 5/5 or reviewers/automation leave actionable comments, invoke the `greploop` skill and work the comments to result.
13. **Merge at 5/5.** When the PR gets a 5/5, merge it automatically unless the user requested PR-open-only delivery.
14. **Close the loop.** Update issue/project status and print the PR URL plus one of: merge result, explicit PR-open handoff, or blocker. If the user queued more issues at the start, loop back to step 1 only after the current issue reaches a terminal completion state. If they named only this one, stop and wait.

## When to escalate back to planning

If mid-execution you discover any of:

- A hidden assumption that changes the interface
- An acceptance criterion that can't be tested through the public interface
- A new module is needed that wasn't in the plan

...stop, add a comment on the issue describing the discovery, and tell the user to run `grill-with-docs` skill (for ambiguity) or `improve-codebase-architecture` (for structural surprises) before continuing. The planning skills exist for exactly this moment.
