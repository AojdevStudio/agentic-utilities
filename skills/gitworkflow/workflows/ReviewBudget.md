# Review budget

the user's policy, 2026-09-14. Apply before starting or resuming any PR review,
remediation, babysitter, or reviewer queue, including delegated agents.

## Limit

Allow an initial review and at most two fix-and-review rounds per PR. One round
is a batch of verified fixes followed by verification and review of the resulting
head. CI repairs during remediation consume the same budget. Batch known blockers
together. A push, rebase, new reviewer, cron tick, or session restart does not
reset the count. Only the user can explicitly authorize additional rounds.

Before editing, record the round as started in
`~/.local/state/pr-review-budget/<owner>/<repo>/<pr>.json`. Keep the PR identity,
round count, input and output SHAs, finding dispositions and evidence, review wait
start time, status, and any explicit extension. The orchestrator owns this record
and passes it to reviewers. Resume an interrupted round from that record. If it
is missing, reconstruct prior rounds from PR commits and review history before
starting work. If the count cannot be established, stop and report the uncertainty.

## Findings

Fix only confirmed blockers within the agreed PR scope. Require a concrete
failure path or reproduction showing why the change is necessary. Defer cosmetic
suggestions, optional refactors, and unrelated improvements with a brief reason.
Reject false positives with evidence. Apply existing authorization rules before
posting replies or resolving threads.

If a reviewer repeats an addressed finding without new evidence, or contradicts
earlier advice, stop the automated loop and report the conflicting claims and
evidence. A regression proven on the current head is a new blocker, subject to
the remaining budget. Never change code merely to raise a score.

## Stop and report

After the second round's verification and review, stop. Stop earlier on a repeated
or contradictory finding, or after 20 minutes waiting for one head's review or
required checks. Persist the wait start time across ticks and restarts.

At every stop, cancel the PR's cron, watcher, and delegated reviewer work. Mark
the record stopped before handing control back. Queue workers skip stopped PRs
until the user explicitly extends their budget. Never launch an automatic third round.

Report the current SHA, rounds used, checks that ran, remaining blockers and
deferred suggestions, and a merge recommendation. Keep the recommendation separate
from actual merge eligibility. Reaching the limit does not waive required checks,
the [merge policy](MergePolicy.md), or merge authorization. If a score is missing
or below the required threshold, report that gate as unmet and stop spending.
