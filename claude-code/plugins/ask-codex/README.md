# ask-codex

OpenAI Codex CLI integration for Claude Code. Hand off code analysis, refactoring, and automated edits to the Codex CLI without leaving your Claude Code session.

## What it does

Auto-activates when you mention "codex", "codex cli", "codex exec", "codex resume", or "ask codex". The skill:

- Prompts for **model** (`gpt-5.4` / `gpt-5.3-codex` / `gpt-5.4-nano`) and **reasoning effort** (`high` / `medium` / `low`) in a single batched question.
- Picks the right **sandbox mode** for the task: `read-only` (default, analysis), `workspace-write` (local edits), or `danger-full-access` (network-enabled).
- Always passes `--skip-git-repo-check` and pipes stderr to `/dev/null` so Codex's thinking tokens don't pollute the chat.
- Supports **session resumption** via `codex exec resume --last` — the resumed session inherits the original model + reasoning + sandbox.
- Asks for permission before invoking high-impact flags (`--full-auto`, `--sandbox danger-full-access`).

## Prerequisites

The `codex` CLI must be installed and authenticated on the host:

```bash
# install (one-time)
npm install -g @openai/codex
# or follow OpenAI's current install instructions

# verify
codex --version
```

Without `codex` on PATH the skill will surface the failure and stop.

## Trigger phrases

- "ask codex to review …"
- "use codex to refactor X"
- "codex resume"
- "run this through codex"

## Safety

The skill defaults to `read-only` sandbox and asks before escalating to `workspace-write` or `danger-full-access`. Resume operations don't auto-pass new flags — the user must explicitly request a model/effort change.

## License

MIT — see repository LICENSE.
