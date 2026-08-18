---
name: adversarial-review
description: Deep implementation review that hunts for real bugs by sending a heavyweight external reviewer (the codex frontier model or Opus) actual file contents, not summaries. Covers implementations only (code, configs, scripts, pipelines). USE WHEN adversarial review, review this implementation, audit my code, stress test this, find problems with this, ship-readiness review, is this ready to ship, check this against the plan. NOT FOR plans and designs before implementation exists (use RedTeam for adversarial plan critique, or grilling for a collaborative interview).
metadata:
  author: ossie
  category: engineering
  lanes: [claude, codex, pi]
---

# Adversarial Review

Sends a deeply structured adversarial prompt to a heavyweight reviewer (the codex frontier model or Opus 4.7) to catch real bugs before they hit production. The reviewer reads actual file contents, not summaries, and returns a trinary verdict with file:line citations and a prioritized fix list.

Distinct from RedTeam (which adversarially challenges plans and ideas with parallel expert attackers) and grilling (which interviews the user about designs). This skill reviews working or near-complete code against its own specification.

## Workflow

### Step 1 — Gather inputs

Use `AskUserQuestion` with these four questions in a single prompt:

1. **Target directory** — What directory should the reviewer work in? (e.g., `~/Projects/arbol/v3`)
2. **Plan or spec file** — Path to the spec, plan, or design doc to review against. If none, the reviewer will assess internal consistency instead.
3. **Reviewer** — Which model? Options:
   - the codex frontier model (recommended — deepest code reasoning, runs locally)
   - `opus 4.7` (alternative — best for architecture-level concerns)
4. **Review areas** — Provide 4–10 numbered areas to focus on (e.g., "1. Cron scheduling, 2. File path assumptions, 3. Error handling"). If unspecified, use the 8 default areas from the prompt template.

### Step 2 — Build the prompt

Load `references/prompt-template.md` and fill in:
- `{{TARGET_DIR}}` — from input 1
- `{{PLAN_FILE}}` — from input 2 (or "no plan file — review for internal consistency")
- `{{REVIEW_AREAS}}` — from input 4 (or the 8 defaults in the template)

### Step 3 — Invoke the reviewer

**For the codex frontier model (default):** the model comes from `~/.codex/config.toml` (the single authority; never pinned here; see `ask-codex`).
```bash
codex exec --skip-git-repo-check \
  --config model_reasoning_effort="high" \
  --sandbox read-only \
  -C {{TARGET_DIR}} \
  "{{FILLED_PROMPT}}" 2>/dev/null
```

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

## Reference

| Topic | File |
|-------|------|
| Full adversarial prompt template | `references/prompt-template.md` |

## Constraints

- Always use `--sandbox read-only` — this skill reads, never writes
- Always suppress stderr with `2>/dev/null` unless the user asks for thinking tokens
- Never summarize findings without showing at least one file:line citation per finding
- After the review, offer `codex resume` if using the codex frontier model
