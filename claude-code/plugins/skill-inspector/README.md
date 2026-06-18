# skill-inspector

Security-inspect an AI agent skill (yours or a third party's) before you install or trust it. Wraps the [`skillspector`](https://pypi.org/project/skillspector/) scanner and renders its hundreds of raw findings into a verdict a human can actually read: SAFE / CAUTION / DO_NOT_INSTALL, a plain-English threat table, the findings worth checking first, and where they cluster.

## What it does

1. **Scans** a skill (Git URL, local dir, zip, `.md`, or file URL) with `skillspector`, capturing JSON + stderr.
2. **Renders** a chat-ready Markdown report via the bundled `render_report.py`.
3. **Explains the verdict honestly:**
   - The score is **capability-clustered, not a severity sum**: N hits of one capability count as one issue, weighted by confidence and LLM-assessed intent, combined with diminishing returns so finding count alone can't pin the score to 100.
   - A **confirmed source-to-sink exfiltration chain** (taint rules TT3/TT4/TT5) is the categorical DO_NOT_INSTALL trigger. That is the line between "reads X" and "steals X".
   - It separates **capability present** (code that can read credentials) from **confirmed theft** (a taint chain where data actually leaves), and gives posture-aware guidance (share vs. run-it-yourself-isolated).
   - It **flags degraded (static-only) scans loudly**, so an unreliable verdict is never mistaken for a trustworthy one.

## Trigger phrases

- "is this skill safe?"
- "inspect this skill" / "scan this skill"
- "check this skill for malware"
- "skillspector"
- sharing a GitHub link to a skill/agent and asking about the risk

## Modes

- **Deep (default):** static + YARA + LLM intent classification. Needs an LLM provider in the environment. Clears the false positives that static-only floods produce.
- **Fast (`--no-llm`):** static + YARA only. No keys, deterministic, always works. Expect false positives; not a final verdict.

## Bundled content

```
skills/skill-inspector/
├── SKILL.md                     # workflow, how to read the verdict, gotchas
└── scripts/render_report.py     # JSON report -> chat-ready Markdown
```

## Prerequisites

- **`skillspector` on PATH.** Install with `uv tool install skillspector` (or `pipx install skillspector` / `pip install skillspector`), or run ad-hoc via `uvx skillspector`.
- **For deep scans only:** an LLM provider configured via environment (`SKILLSPECTOR_PROVIDER`, the matching API key, `SKILLSPECTOR_MODEL`). Fast scans need none of this.
- **Python 3** for the render script.

## License

MIT. See repository LICENSE.
