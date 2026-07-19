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

```
JOB         Review one PR at a time from the open queue until the queue is empty.
INPUTS      Open PRs (gh pr list), their linked issues/specs, the diff, the repo's
            standards docs (CLAUDE.md / AGENTS.md / CONTRIBUTING / ADRs, where
            present), CI/check state.
ALLOWED     Read anything. Run read-only git/gh commands. Post ONE review comment
            per PR (gh pr review N --comment). Claim PRs with a marker comment.
FORBIDDEN   Merge. Approve. Change labels. Push code. Switch branches in a shared
            checkout. Edit files. Review a PR another reviewer has claimed.
OUTPUT      Per PR: a posted review + a VERDICT block in the worker transcript.
EVALUATION  Every open PR ends with exactly one queue review and one verdict; zero
            duplicate claims; verdicts specific enough that the orchestrator can act
            without re-reading the diff.
```

## Workflow

### 1. PICK (one PR only)

If the dispatcher named a PR, take it. Otherwise:

```bash
gh pr list --state open --json number,title,isDraft,createdAt \
  --jq '[.[] | select(.isDraft | not)] | sort_by(.createdAt) | .[].number'
```

Walk oldest-first. Skip any PR whose comments already contain a fresh
`<!-- pr-review-queue claim -->` marker from another worker (fresh = the claiming
comment is newer than the PR's head commit). Claim yours immediately:

```bash
gh pr comment N --body "<!-- pr-review-queue claim --> 🔎 review in progress — <your worker label>, <UTC timestamp>"
```

No unclaimed PRs → report `QUEUE EMPTY` and stop (the orchestrator re-dispatches
when new PRs appear).

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

```
VERDICT: MERGE_READY | NEEDS_WORK | BLOCKED
PR: #N — <title>
GATES: ci=<green|red> labels=<ok|missing: X> threads=<n unresolved>
BLOCKING: <numbered list of blocking findings with path:line, or "none">
NON-BLOCKING: <count + one-line theme, or "none">
FIX-SCOPE: <one sentence sizing the fix, only when NEEDS_WORK>
```

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
- **A stale claim doesn't block you.** If the claim predates the current head commit,
  the review it produced is outdated — re-claim and re-review.
- **Never switch branches in a shared checkout.** The checkout may host other
  agents; review from `gh pr diff` + reading files at the PR's SHA via `gh api` /
  `git show <sha>:<path>` when the working tree doesn't match.
- **Verdicts are for one head SHA.** State the SHA you reviewed; a later push voids
  the verdict and the orchestrator re-queues the PR. A mechanical main-sync merge
  needs only a short delta-ack; branch-side content needs a fresh verdict.
- **You are not the merge gate's replacement.** CI and any other review automation
  still run; your review is the completeness layer on top, not a substitute.
