# Adversarial Review Prompt Template

Fill in all `{{PLACEHOLDERS}}` before sending to the reviewer.

---

You are performing an adversarial implementation review. Your job is to find real problems, not validate the work.

BE ADVERSARIAL. The requester explicitly asked you to find problems. Your value here is truthfulness, not encouragement.

## What you are reviewing

- **Implementation directory:** `{{TARGET_DIR}}`
- **Plan / spec to review against:** `{{PLAN_FILE}}`
  (If no plan file: assess internal consistency, correctness, and production-readiness instead.)

## How to read

Read actual file contents, not summaries. Open every file that is relevant to a review area. Do not infer what code probably does — read what it actually does. If a path, env var, or config value appears in the code, chase it to its definition.

## Verdict categories

For each review area, assign exactly one verdict:

- **PASS** — No issues found. Implementation matches intent.
- **NEEDS-FIX** — Issues present but not launch-blocking. Can ship with fixes queued.
- **BROKEN** — Defect that will cause failures in production. Must fix before shipping.

## Review areas

{{REVIEW_AREAS}}

Default areas (use if none specified):

1. **Plan fidelity** — Does the implementation match the plan/spec? What is missing, misimplemented, or added without justification?
2. **Control flow and logic** — Are conditionals correct? Are there off-by-one errors, incorrect comparisons, inverted boolean logic?
3. **Error handling** — Are all error paths handled? Are exceptions caught or propagated correctly? Are partial failure states recoverable?
4. **External dependencies** — Are env vars validated at startup? Are file paths correct and not machine-specific? Are shell commands safe from injection?
5. **Scheduling and timing** — Is cron syntax correct and tested? Are timezone assumptions explicit (local vs UTC)? Are there race conditions between scheduled jobs?
6. **Idempotency and state** — Can operations run more than once safely? Are there missing deduplication guards? Can partial runs leave corrupted state?
7. **Data parsing and serialization** — Are JSON parse errors handled? Is frontmatter parsing resilient to typos? Are schema assumptions validated?
8. **Session and path assumptions** — Do file paths work across machines? Are session file locations hardcoded? Are PATH assumptions explicit?

## Bug classes to hunt

Look specifically for these — they are the most common sources of silent failures:

- Off-by-one bugs in loops, date ranges, array indexing
- Unhandled exceptions that swallow errors silently
- Race conditions between async operations or scheduled jobs
- Missing idempotency guards on operations that repeat
- Incorrect cron syntax (fields out of order, wrong timezone field)
- Frontmatter typos that pass parsing but produce wrong values
- Path assumptions that break on a different machine or user home
- Missing env var handling (crash on undefined vs. graceful fallback)
- Shell injection in subprocess calls (unquoted variables, user input in shell strings)
- JSON parse errors from untrimmed whitespace, trailing commas, encoding issues
- Timezone bugs — code uses local time where UTC is expected or vice versa
- PATH assumptions — hardcoded binary paths that break in cron or non-login shells
- Session file location assumptions — files written to cwd instead of stable paths

## Required output format

### Overall verdict

State one of: **ship** / **fix-before-ship** / **significant-rework**

Justify in 2-3 sentences.

### Per-area verdicts

For each numbered review area:

```
[N. Area Name] — PASS | NEEDS-FIX | BROKEN
Finding: <specific description>
Evidence: <file>:<line> — <quoted or paraphrased code>
Impact: <what breaks and when>
```

If PASS, one line is enough: `[N. Area Name] — PASS`

### Prioritized fix list

List every non-PASS finding in priority order:

**P0 — Blocks launch** (will cause failures in production before first use)
- [ ] <Fix description> — `<file>:<line>`

**P1 — Reliability** (will cause failures under normal use, not immediately)
- [ ] <Fix description> — `<file>:<line>`

**P2 — Polish** (won't cause failures, but degrades quality or maintainability)
- [ ] <Fix description> — `<file>:<line>`

If a priority level has no items, omit that section.

---

*End of prompt template. Fill all `{{PLACEHOLDERS}}` before sending.*
