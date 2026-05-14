---
name: adversarial-review
description: Deep implementation review that hunts for real bugs. Use when Ossie asks for adversarial review, implementation audit, ship-readiness review, stress test this, review this against the spec, or find problems with this code. This skill is for implemented code, configs, scripts, and pipelines.
---

# Adversarial Review

Use Pi's `adversarial_review` tool to run a second-pass, read-only implementation audit with a separate reviewer model inside Pi.

This is for **implementations**.

- Use `the-fool` for challenging plans before code exists.
- Use `grill-me` when the user wants questioning and design pressure-testing.
- Use normal inline review only for light checks.
- Use this skill when the user wants the toughest audit before shipping.

## Workflow

### 1. Gather only what is missing

Figure out:

1. **Target directory**
   - Default to the current working directory when the user is clearly referring to the current repo.
   - Ask only if the target is unclear.
2. **Plan or spec file**
   - Use it if the user gave one.
   - If none exists, review for internal consistency and production-readiness.
3. **Reviewer model**
   - Prefer a reviewer model different from the current one when possible.
   - Let the tool pick the best available default if the user does not care.
4. **Review areas**
   - If the user gives focused areas, pass them through.
   - Otherwise use the default 8 areas from `references/prompt-template.md`.

### 2. Run the Pi review tool first

Call `adversarial_review` with:

- `targetDir`
- optional `planFile`
- optional `reviewerModel`
- optional `reviewAreas`

The tool normally enforces the right shape:

- separate reviewer model inside Pi
- read-only tools only
- no file edits
- required file:line citations for non-PASS findings

### 2b. Claude Code fallback / explicit Claude request

If the user specifically asks to "ask Claude", "ask Claude Code", "use Opus", or if the Pi `adversarial_review` tool returns no grounded report, use Claude Code directly instead of treating the failed tool call as a valid review.

Build the filled prompt from `references/prompt-template.md`, then run Claude Code headlessly from the target directory with read-only tools:

```bash
claude --print --model opus --effort high \
  --add-dir "{{TARGET_DIR}}" \
  --add-dir "$(dirname "{{PLAN_FILE}}")" \
  --tools "Read,Grep,Glob,Bash" \
  --disallowedTools "Edit,Write,MultiEdit,NotebookEdit" \
  "{{FILLED_PROMPT}}"
```

When intentionally delegating to the Claude-side adversarial-review skill from `~/.agents/skills/adversarial-review`, use its headless form:

```bash
claude --print --model opus --effort high \
  --add-dir "{{TARGET_DIR}}" \
  --add-dir "$(dirname "{{PLAN_FILE}}")" \
  --tools "Read,Grep,Glob,Bash" \
  --disallowedTools "Edit,Write,MultiEdit,NotebookEdit" \
  "Run /adversarial-review headlessly with: target={{TARGET_DIR}}, plan={{PLAN_FILE_OR_NONE}}, reviewer=opus 4.7, areas=[{{REVIEW_AREAS}}]"
```

Notes:

- Use `--dangerously-skip-permissions` only inside disposable/sandboxed worktrees when the user explicitly wants that mode; prefer tool allow/deny lists first.
- If `{{PLAN_FILE}}` is omitted, omit the second `--add-dir` and pass `plan=none`.
- The Claude result is valid only if it contains file:line citations for non-PASS findings. If Claude cannot read files or returns an environment/tooling failure, report that as a failed review, not as implementation findings.

### 3. Present the result cleanly

Always present:

- **Overall verdict** — ship / fix-before-ship / significant-rework
- **Per-area verdicts** — PASS / NEEDS-FIX / BROKEN
- **Prioritized fixes** — P0, then P1, then P2

Do not flatten or soften the review. Keep the citations.

### 4. If P0s exist, switch to execution mode

If the review surfaces P0 items, ask whether to fix them immediately.
If yes, move straight into implementation work on those items.

## Constraints

- Never use write/edit/bash for this review pass unless the user explicitly changes the task from review to implementation.
- Never summarize non-PASS findings without at least one file:line citation per finding.
- Keep the review adversarial and truth-seeking, not encouraging.
- This skill audits implementations, not ideas.
