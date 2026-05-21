---
name: adversarial-review
description: "Deep implementation review that hunts for real bugs. Use when the user asks for adversarial review, review this implementation, audit my code, stress test this, find problems with this, ship-readiness review, is this ready to ship, check this against the plan, check this against the spec, find bugs in this, implementation audit. This skill is for IMPLEMENTATIONS — code, configs, scripts, pipelines. For plans and designs before implementation, use a plan-review or design-challenge skill instead."
---

# Adversarial Review

Sends a deeply structured adversarial prompt to a heavyweight reviewer (codex gpt-5.5, Gemini 3 Pro, or Opus 4.7) to catch real bugs before they hit production. The reviewer reads actual file contents, not summaries, and returns a trinary verdict with file:line citations and a prioritized fix list.

Distinct from plan-review and design-challenge skills. This skill reviews working or near-complete code against its own specification.

## Workflow

### Step 1 — Gather inputs

**Non-interactive invocation (CLI / external callers):** If the four inputs below are already present in the invoking prompt, skip `AskUserQuestion` entirely and proceed to Step 2. This is how external tools (e.g., Codex calling `claude --print`) drive the skill headlessly. Accept any of these forms:

- Inline key=value pairs: `target=<dir>, plan=<file|none>, reviewer=<codex gpt-5.5|gemini 3 pro|opus 4.7>, areas=[1. X, 2. Y, ...]`
- A YAML/JSON block with keys `target_dir`, `plan_file`, `reviewer`, `review_areas`
- Any unambiguous natural-language equivalent that supplies all four

If `plan_file` is omitted or "none", review for internal consistency. If `review_areas` is omitted, use the 8 defaults from the prompt template. If `reviewer` is omitted, default to `codex gpt-5.5` (or the value from `.claude/adversarial-review.local.md` if present). If `target_dir` is missing, that is the only field that must be asked for — request just that one and proceed.

**Interactive invocation (default):** Use `AskUserQuestion` with these four questions in a single prompt:

1. **Target directory** — What directory should the reviewer work in? (e.g., `/path/to/your/project`)
2. **Plan or spec file** — Path to the spec, plan, or design doc to review against. If none, the reviewer will assess internal consistency instead.
3. **Reviewer** — Which model? Options:
   - `codex gpt-5.5` (recommended — deepest code reasoning, runs locally via `codex` CLI)
   - `gemini 3 pro` (alternative — strong at cross-file analysis; requires your Gemini CLI setup)
   - `opus 4.7` (alternative — best for architecture-level concerns)
4. **Review areas** — Provide 4–10 numbered areas to focus on (e.g., "1. Cron scheduling, 2. File path assumptions, 3. Error handling"). If unspecified, use the 8 default areas from the prompt template.

**Example headless call from Codex:**
```bash
claude --print --dangerously-skip-permissions \
  "Run /adversarial-review headlessly with: target=/path/to/your/project, plan=docs/spec.md, reviewer=codex gpt-5.5, areas=[1. Auth, 2. Migrations, 3. Error paths]"
```

### Step 2 — Build the prompt

Load `references/prompt-template.md` and fill in:
- `{{TARGET_DIR}}` — from input 1
- `{{PLAN_FILE}}` — from input 2 (or "no plan file — review for internal consistency")
- `{{REVIEW_AREAS}}` — from input 4 (or the 8 defaults in the template)

### Step 3 — Invoke the reviewer

**Fresh-context requirement (CRITICAL):** the reviewer must operate in a clean context window — never inside the same session that performed the implementation. A reviewer running atop accumulated implementation tokens is in the "dumb zone" (degraded attention) and will be less capable than the implementer was at the end of its run. Codex and Gemini calls satisfy this automatically because they spawn external processes; the Opus path must always spawn a fresh subagent (never inline). If this skill is invoked from within an implementation session, do not attempt to "review in place" — spawn a sub-agent unconditionally so the reviewer starts cold.

**For codex gpt-5.5 (default):**
```bash
codex exec --skip-git-repo-check -m gpt-5.5 \
  --config model_reasoning_effort="high" \
  --sandbox read-only \
  -C {{TARGET_DIR}} \
  "{{FILLED_PROMPT}}" 2>/dev/null
```

**For Gemini 3 Pro:**
Invoke your Gemini CLI of choice with the filled prompt and target directory as context. See Prerequisites in the plugin README for setup guidance.

**For Opus 4.7:**
Spawn a subagent with model `opus` and pass the filled prompt directly.

### Step 4 — Parse and present output

Extract from the reviewer's response:
- **Overall verdict**: ship / fix-before-ship / significant-rework
- **Per-area verdicts**: PASS / NEEDS-FIX / BROKEN for each numbered area
- **Prioritized fix list**: P0 (blocks launch) → P1 (reliability) → P2 (polish)

Present as a clean summary with the fix list ordered by priority.

### Step 5 — Offer P0 handoff

If there are any P0 items, ask: "Should I hand these P0 items to an Engineer agent for immediate fixes?"
If yes, spawn an Engineer subagent with the P0 list and the target directory.

**Headless mode:** Skip the offer. Print the verdict + prioritized fix list and exit. The external caller decides what to do with P0s.

## Reference

| Topic | File |
|-------|------|
| Full adversarial prompt template | `references/prompt-template.md` |

## Constraints

- Always use `--sandbox read-only` — this skill reads, never writes
- Always suppress stderr with `2>/dev/null` unless the user asks for thinking tokens
- Never summarize findings without showing at least one file:line citation per finding
- After the review, offer `codex resume` if using codex gpt-5.5 and the user wants to drill into findings
