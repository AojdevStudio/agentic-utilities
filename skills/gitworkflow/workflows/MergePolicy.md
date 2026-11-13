# Review bot merge policy

Every AojdevStudio PR is gated by a **review bot**: an automated reviewer whose
verdict on the exact current head is a required merge condition. Which bot is
active is a repository fact, read from the PR's check-runs, not a fixed name.

| Bot | Pass signal on the exact current head | Status |
| --- | --- | --- |
| Pullfrog (`pullfrog` GitHub App) | The `pullfrog-approval` check-run concludes `success`. If the repo has not enabled that check, the `pullfrog` run check plus a review comment with no blocking findings stands in. | Active gate since 2026-09-15 |
| Open Code Review (`open-code-review.yml`) | None. Inline comments only. | Advisory |
| CodeRabbit, Greptile, Codex | None unless the repo's branch rules require them. | Advisory |

A PR passes the review-bot gate when the active bot's pass signal is present on
the current head. A verdict on an older head, a missing verdict, or a failing
one does not pass; wait for the bot rather than proceeding.

**Exception: an approved head that needs only a conflict fix** (the user,
2026-10-03). The bot's approval carries forward to the merge commit when all of
these hold. Do not spend another bot review on it.

- The bot's pass signal or an APPROVED review exists on the prior head.
- The only new commit merges the base branch into the PR and resolves its
  conflicts. It adds no other change.
- The agent resolves the conflicts, proves the resolution keeps both sides'
  intent with a second-model review of the merge commit, and records the
  review-gate receipt.
- Required CI passes on the merge commit.

If the bot starts a run on the merge commit anyway, cancel it. State the
carried-forward approval and the resolution evidence in a PR comment before
merging.

Before merging, verify all of the following on the same head:

1. The review bot's pass signal (table above).
2. Required CI passes, the agreed scope is satisfied, and every actionable
   finding is addressed. Review-bot threads carry an evidence-backed reply and
   are resolved; other threads are resolved or explicitly dispositioned.
3. the user has authorized merging this work. A passing verdict is not authorization.

A check name such as `Review 5/5` is a label, not the operator's threshold.
If GitHub actually requires a failing check, report the configuration conflict;
this policy does not authorize bypassing branch protection. Never describe
unrun verification as passing.

A later explicit instruction from the user changes this policy. Older repository
text, copied prompts, and historical incident notes do not.
