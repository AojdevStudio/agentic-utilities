---
name: pr-review-queue
description: Standing PR-review loop for dedicated reviewer workers in a multi-agent fleet. Picks the oldest unclaimed open PR, runs a completeness review (two-axis code review + adversarial bug hunt + gate check), posts the review on the PR, emits a structured VERDICT for the orchestrator, then moves to the next PR. Use when a worker is assigned to "review PRs from the queue", "run the review workflow", or "/pr-review-queue [PR#]". Works in any git repo with the gh CLI.
---

# PR Review Queue

A fleet runs one or more standing reviewer workers. Each drains the open-PR queue
**one PR at a time, to completion**, and reports a machine-readable verdict so the
orchestrator can merge good PRs and dispatch fixes for bad ones. Reviewers never
merge and never fix — they judge.

## Scope

The queue is the repo of your current checkout (`gh` resolves it implicitly). To
review a PR in another repo, the dispatcher must name it explicitly; then use
`GH_REPO=<owner>/<repo>` on every `gh` command and read files via `gh api` /
`gh pr diff` or a temp clone — never by switching an existing checkout.

## JOB card

```text
JOB         Review one PR at a time from the open queue. Solo/one-shot dispatch
            stops when the queue is empty; a standing fleet lane polls instead.
INPUTS      Open PRs (gh pr list), their linked issues/specs, the diff, the repo's
            standards docs (CLAUDE.md / AGENTS.md / CONTRIBUTING / ADRs, where
            present), CI/check state.
ALLOWED     Read anything. Run read-only git/gh commands. Post ONE review comment
            per PR (gh pr review N --comment). Claim PRs with a marker comment.
FORBIDDEN   Merge. Approve. Change labels. Push code. Switch branches in a shared
            checkout. Edit files. Review a PR another reviewer has claimed. Post a
            PR comment as a heartbeat.
OUTPUT      Per PR: a posted review + a VERDICT block (with HEAD/SYNC) in the
            worker transcript. Fleet idle periods: a HEARTBEAT line, not a comment.
EVALUATION  Every open PR ends with exactly one queue review and one verdict; zero
            duplicate claims; verdicts specific enough that the orchestrator can act
            without re-reading the diff.
```

## Workflow

### 1. PICK (one PR only)

If the dispatcher named a PR, take it. Otherwise fetch the *complete* open
queue: the default 30-item `gh pr list` limit silently truncates older PRs
and can misreport `QUEUE EMPTY`:

```bash
gh pr list --state open --json number,title,isDraft,createdAt --limit 1000 \
  --jq '[.[] | select(.isDraft | not)] | sort_by(.createdAt) | .[].number'
```

Walk oldest-first. For each candidate, capture its current head before
claiming: the claim is bound to this exact commit, not to comment timing.

```bash
head_sha=$(gh pr view N --json headRefOid --jq .headRefOid)
```

Skip the PR if its comments already contain a `<!-- pr-review-queue claim
head=$head_sha -->` marker from another worker. "Fresh" now means *bound to
this exact head*, not "posted recently." No clock or comment-ordering
assumption is required. Otherwise claim it:

```bash
gh pr comment N --body "<!-- pr-review-queue claim head=$head_sha --> 🔎 review in progress — <your worker label>, <UTC timestamp>"
```

Then re-read to confirm you actually won the claim: another worker may have
posted for the same head in the gap between your read and your comment.

```bash
gh pr view N --json headRefOid,comments --jq \
  "[.comments[] | select(.body | test(\"pr-review-queue claim head=$head_sha\"))] | sort_by(.createdAt) | .[0].body"
```

Proceed only if (a) the first matching claim comment is yours, and (b) the
PR's `headRefOid` is still `$head_sha`. If another worker's claim for this
same head sorts first, or the head has already moved, drop the claim
deterministically (do not review) and move to the next candidate PR.

No unclaimed PRs:

- **Solo / one-shot dispatch:** report `QUEUE EMPTY` and stop.
- **Fleet mode (standing reviewer lane):** do not terminate. Enter a bounded
  polling loop with backoff (30s, 60s, 120s, capping at 5 min) that rechecks
  (a) `gh pr list` for new or updated open PRs and (b) any PR you've already
  verdicted whose `headRefOid` has since moved or whose review threads
  changed. Resume at the top of this step the moment either check finds
  work. Emit a heartbeat to your own worker transcript each poll (e.g.
  `HEARTBEAT: queue empty, next check in <n>s`). Never a PR comment: PR
  comments are for claims and reviews only. Keep polling until the control
  pane stops you (a new dispatch or an explicit shutdown signal).

### 2. REVIEW (the completeness pass)

If a `code-review` skill is available, run its two-axis process (Standards + Spec)
against the PR's merge-base. Otherwise run both axes yourself:

- **Standards axis:** does the change follow this repo's documented conventions
  (style, architecture, naming, error handling) and general code-quality baselines?
- **Spec axis:** does the change do what the linked issue / PR body promises?

On top of the two axes, verify completeness:

- **Spec closure:** every acceptance criterion in the linked issue (or spec comment)
  is either implemented or explicitly declared out of scope in the PR body.
  Unstated omissions are findings.
- **Adversarial pass:** hunt real bugs in the changed hunks — correctness, security,
  silent failures (empty catch, swallowed rejection, fail-open), concurrency, edge
  cases (empty/whitespace input, missing binary, stale state). Read the actual files
  at the cited ranges; never review from the diff summary alone.
- **Test honesty:** new behavior has a test that fails if the behavior breaks; tests
  assert outcomes, not implementation echoes. Flag anything only covered when a
  local-only precondition holds (prebuilt binaries, seeded state, developer dotfiles).
- **Gates:** CI rollup green; every merge gate the repo documents (ownership/lane
  labels, risk labels, changelog or work-log entry requirements) satisfied; zero
  unresolved threads from other review automation.

### 3. POST

One review per PR:

```bash
gh pr review N --comment --body-file <review.md>
```

Structure: `## Standards` and `## Spec` (kept as separate axes — do not merge or
rerank them), then `## Completeness`, `## Adversarial findings`, `## Gates`. Every
finding cites `path:line` you actually read. If nothing blocks, say exactly what
you verified — never a bare LGTM.

### 4. VERDICT (for the orchestrator)

End your worker report with exactly this block:

```text
VERDICT: MERGE_READY | NEEDS_WORK | BLOCKED
PR: #N — <title>
HEAD: <sha>
SYNC: full-review | mechanical-delta-ack
GATES: ci=<green|red> labels=<ok|missing: X> threads=<n unresolved>
BLOCKING: <numbered list of blocking findings with path:line, or "none">
NON-BLOCKING: <count + one-line theme, or "none">
FIX-SCOPE: <one sentence sizing the fix, only when NEEDS_WORK>
```

- `HEAD`: the exact SHA you reviewed. The orchestrator diffs this against the
  PR's current `headRefOid`; a mismatch means a later push voided this verdict
  and the PR must be re-queued for a fresh review.
- `SYNC`: `full-review` when you ran the complete two-axis + adversarial pass
  at `HEAD`; `mechanical-delta-ack` when this push was only a mechanical
  main-sync merge with no branch-side content change, and you're carrying
  forward the prior verdict's findings rather than re-reviewing from scratch.
- `MERGE_READY` — all gates green and zero blocking findings. The orchestrator merges.
- `NEEDS_WORK` — blocking findings exist; FIX-SCOPE tells the orchestrator what to
  dispatch to an implementer.
- `BLOCKED` — can't complete the review (missing spec, red CI unrelated to the diff,
  provider outage). Say what unblocks it.

### 5. LOOP

Return to step 1 for the next unclaimed PR. One at a time; never parallelize PRs
within one worker — depth over throughput is the point of having several of you.

## Gotchas

- **Claim before reviewing.** Two workers on one PR wastes the second worker and
  splits findings across two comments. The claim marker is the mutex.
- **A stale claim doesn't block you.** A claim bound to an older `headRefOid` than
  the PR's current head is stale, not a lock: capture the new head, re-claim, and
  re-review.
- **Never switch branches in a shared checkout.** The checkout may host other
  agents; review from `gh pr diff` + reading files at the PR's SHA via `gh api` /
  `git show <sha>:<path>` when the working tree doesn't match.
- **Verdicts are for one head SHA.** The `HEAD` field is what makes this
  machine-checkable: a later push means the PR's `headRefOid` no longer matches
  your `HEAD`, and the orchestrator re-queues it. Set `SYNC: mechanical-delta-ack`
  only for a mechanical main-sync merge with no branch-side content change;
  anything else needs `SYNC: full-review`.
- **You are not the merge gate's replacement.** CI and any other review automation
  still run; your review is the completeness layer on top, not a substitute.
