# Herdr Fleet

`herdr-fleet` helps one control pane launch or rebuild a project-scoped Herdr fleet. It asks the user for the roster, previews the pane map, reuses ownership-proven workers, and creates only confirmed missing panes. There is no default worker count or fixed worker roster.

See [SKILL.md](SKILL.md) for the agent contract, [launch-fleet.md](launch-fleet.md) for intake and reconciliation, and [protocols.md](protocols.md) for the live event loop.

## Roster intake

Intake is a guided wizard: fleet size, then per-worker harness (claudex / claude / codex / pi / custom), label, role, and assignment — with a paste-lines escape for large rosters.

Each worker is user-selected:

- label;
- launch command;
- role (`implementer`, `reviewer`, or another role);
- optional assignment or lane constraint;
- pane placement.

The current control pane is fixed and is not included in the worker count. Launch and rebuild require an `AskUserQuestion` roster preview and confirmation before worker-pane mutation.

## Launcher menu

These are launcher options, not a default roster:

| Command | Routed model | Effort |
| --- | --- | --- |
| `claudex` | GPT-5.6 Sol | xhigh |
| `claudex --model fable` | GPT-5.6 Sol | xhigh |
| `claudex --model opus` | GPT-5.6 Sol | high |
| `claudex --model sonnet` | Grok 4.5 | high |
| `claudex --model haiku` | GPT-5.6 Terra | medium |
| `claude` | Native Claude Code | native model selection |

The roster can also use `pi`, `codex`, or any user-provided command.

Claudex keeps the Claude Code harness—its tools, permissions, skills, hooks, context management, and interface—while routing model traffic through CLIProxyAPI. Ossie's setup article, [The Fable Effect](https://aojdevstudio.me/blog/the-fable-effect/#how-i-ran-gpt-56-sol-inside-the-claude-code-harness), is the canonical setup reference. The article is currently a draft, so the URL may return 404 until publication.

This package intentionally does not duplicate proxy configuration, credentials, local paths, or setup secrets.

## Source of truth

The canonical copy of this skill lives in the publisher's global skills directory (`~/.agents/skills/herdr-fleet`); the publisher's harness skill directories (`~/.pi`, `~/.claude`, `~/.codex`) intentionally symlink to it. This repository is the public snapshot, synced one-way from the canonical copy.
