# adversarial-review

Deep implementation review for Claude Code. One skill, one prompt template — sends a structured adversarial prompt to a heavyweight external reviewer (codex gpt-5.5, Gemini 3 Pro, or Opus 4.7) to catch real bugs before they hit production. Returns a trinary verdict (`ship` / `fix-before-ship` / `significant-rework`) with file:line citations and a P0/P1/P2 fix list.

## What it does

- **Reads actual file contents, not summaries.** The reviewer opens every relevant file and chases every path, env var, and config value to its definition.
- **Trinary verdict with citations.** Every non-PASS finding must include a `file:line` evidence citation — no unanchored opinions.
- **Prioritized fix list.** Findings are bucketed P0 (blocks launch) / P1 (reliability) / P2 (polish), so you know what to fix first.
- **Fresh-context enforcement.** The reviewer always runs in a clean context window — never inline inside an implementation session — so it isn't degraded by accumulated implementation tokens.
- **Headless + interactive.** Works interactively (asks 4 questions) or headlessly (accepts inline key=value pairs, YAML/JSON, or natural language with all 4 inputs pre-supplied). External tools like Codex can drive it as a subprocess.
- **P0 handoff.** In interactive mode, if P0 items are found, the skill offers to spawn an Engineer subagent to fix them immediately.

## Trigger phrases

The skill auto-activates on phrases like:

- "adversarial review" / "review this implementation"
- "audit my code" / "implementation audit"
- "stress test this" / "find problems with this"
- "ship-readiness review" / "is this ready to ship"
- "review against the plan" / "check this against the spec"
- "find bugs in this"

## Bundled content

```
skills/adversarial-review/
├── SKILL.md                        # entry point — workflow + reviewer invocations
└── references/
    └── prompt-template.md          # the adversarial prompt with 8 default review areas
```

## Prerequisites

### Required

- **`codex` CLI on PATH** — The default reviewer path runs:
  ```bash
  codex exec --skip-git-repo-check -m gpt-5.5 \
    --config model_reasoning_effort="high" \
    --sandbox read-only \
    -C <target-dir> \
    "<filled-prompt>"
  ```
  Install via: `npm install -g @openai/codex` (or your platform's equivalent). Authenticate with your OpenAI API key.

### Optional (for alternate reviewer paths)

- **Gemini 3 Pro path** — Requires a working Gemini CLI or wrapper that can accept a long prompt and a working directory. The skill describes the invocation abstractly; you supply the plumbing. Example: a shell script or Claude skill that calls `gemini --prompt "..."`.

- **Opus 4.7 path** — Requires an Opus-capable Claude API setup. The skill spawns a fresh subagent with model `opus`; your Claude Code session must have access to that model tier.

## User configuration (optional)

Create `.claude/adversarial-review.local.md` in any repo you use this skill in to override defaults:

```markdown
# adversarial-review local config

default_reviewer: codex gpt-5.5
codex_model: gpt-5.5
codex_reasoning_effort: high
```

The skill reads this file when present and falls back to the defaults shown in `SKILL.md` otherwise. The most useful override is `default_reviewer` — set it to `gemini 3 pro` or `opus 4.7` to change the default without being asked every time.

## What's NOT in this plugin

- **Gemini CLI / wrapper.** The Gemini 3 Pro reviewer path requires your own Gemini CLI setup. The plugin only describes the invocation pattern — you supply the binary or wrapper.
- **Opus subagent plumbing.** The Opus 4.7 path spawns a Claude subagent; it works out of the box in Claude Code sessions with Opus access, but the model tier must be available in your account.
- **An Engineer agent.** The P0 handoff step spawns a generic Engineer subagent; no pre-built engineer skill is bundled here.

## License

MIT — see repository LICENSE.
