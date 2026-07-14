---
name: skill-inspector
description: Security-inspect an AI agent skill (your own or a third party's) with the skillspector CLI, then deliver a plain-English verdict report in chat (safe / caution / do-not-install, threat breakdown, top findings to check, file hotspots). Use whenever you want to vet, scan, audit, or sanity-check a skill before installing or trusting it, point at a skill repo/folder/zip and ask "is this safe?", say "inspect this skill", "scan this skill", "check this skill for malware", "skillspector", or share a GitHub link to a skill/agent and want to know the risk. Also use to re-read confusing skillspector output.
---

# Skill Inspector

Wraps the [`skillspector`](https://pypi.org/project/skillspector/) security scanner and renders its output as a report a human can actually read. skillspector finds risky patterns in agent skills (data exfiltration, credential access, code execution, prompt injection, persistence, malware signatures) and assigns a 0-100 risk score. Its raw output is hundreds of findings, which is unreadable. This skill collapses that into a verdict, a plain-English threat table, the findings worth checking first, and where they cluster.

## Prerequisites

- **`skillspector` on PATH.** Install with `uv tool install skillspector` (or `pipx install skillspector` / `pip install skillspector`), or run it ad-hoc via `uvx skillspector`. Confirm with `command -v skillspector`.
- **For deep scans only:** a working LLM provider configured via environment: `SKILLSPECTOR_PROVIDER` (e.g. `openai`, `anthropic`, `nvidia`), the matching API key (e.g. `OPENAI_API_KEY` / `ANTHROPIC_API_KEY`), and a `SKILLSPECTOR_MODEL` the provider actually serves. Fast scans need none of this.

## When to reach for which mode

- **Deep (default):** static + YARA + LLM intent classification. The static-only pass floods the report with false positives (security *vocabulary* in a skill's own docs trips the rules), so a fast scan routinely lands a benign skill in DO_NOT_INSTALL and wastes a round-trip. Deep reads intent and clears those. Run deep unless explicitly told to go fast, or the LLM provider is unavailable.
- **Fast (on request, or no provider):** static + YARA only, `--no-llm`. No keys, deterministic, always works. Use for quick checks or when no LLM provider is configured. Expect false positives; do not deliver a fast-scan verdict as final.

## Workflow

Run the scanner, capture JSON + stderr, then render. Use a slug from the target so parallel scans don't collide.

```bash
SKILLSPEC="$(command -v skillspector || true)"
[ -z "$SKILLSPEC" ] && { echo "skillspector not on PATH: see Prerequisites"; exit 1; }

# Render script ships with this plugin.
SKILL_DIR="${CLAUDE_PLUGIN_ROOT}/skills/skill-inspector"
SLUG=<short-name-of-target>          # e.g. agent-reach

# 1. Scan (DEEP/LLM default, no --no-llm). Accepts a Git URL, local dir, zip, .md, or file URL.
#    Deep needs SKILLSPECTOR_PROVIDER + the provider API key + SKILLSPECTOR_MODEL in the env.
#    Use 2>| (not 2>) so a re-scan with the same slug isn't blocked by zsh noclobber.
"$SKILLSPEC" scan "<TARGET>" \
  --format json --output "/tmp/skillspector-$SLUG.json" \
  2>| "/tmp/skillspector-$SLUG.stderr"

# 2. Render the readable report
python3 "$SKILL_DIR/scripts/render_report.py" \
  "/tmp/skillspector-$SLUG.json" --scan-log "/tmp/skillspector-$SLUG.stderr"
```

For a **fast/static** scan, add `--no-llm`. The scan exits non-zero whenever findings exist, which is normal, not a failure. Always render the JSON regardless of exit code.

## Present the report

The renderer prints chat-ready Markdown. **Paste it straight into the reply.** Don't re-summarize or re-format it; that's the deliverable. Then add at most one line of your own read (for example: "the YARA hits on a CHANGELOG and a PNG are likely false positives, the real concern is the credential-access cluster in `cli.py`").

## How to read the result (so you can explain it)

- **Verdict** is derived from the score band: SAFE (0-14), CAUTION (15-49), DO_NOT_INSTALL (50+). It's a heuristic, not a human judgment.
- **The score is capability-clustered, not a severity sum.** Findings are grouped into capability clusters (one cookie-read capability = one issue, not N hits), each weighted by confidence × LLM-assessed intent (malicious/negligent/benign), combined with diminishing returns so finding *count* no longer pins the score to 100. A **confirmed source→sink exfiltration chain** (taint rules TT3/TT4/TT5) is the categorical DO_NOT_INSTALL trigger. That's the line between "reads X" and "steals X".
- **A 100/100 almost always means a degraded (static-only) scan**, not a maximally-evil skill. With no LLM, findings get no intent and many static clusters drive the score up. The renderer flags this loudly; treat a degraded verdict as unreliable and re-run with a working provider.
- **Capability is not confirmed theft.** The report separates "capability present" (code that *can* read credentials) from "confirmed exfiltration" (a taint chain where data actually leaves). Most credential-access findings are the former: dual-use, often safe to run sandboxed. Judge each cluster; don't treat capability as proven malice.
- **Posture matters.** The "Using it safely" section reframes the same findings for sharing-with-others vs. running-it-yourself-isolated. A capability-only skill is usually fine to run in a container/VM that's denied the real secrets.

## Gotchas

- **A stale or empty env var silently degrades a deep scan.** If `SKILLSPECTOR_MODEL` names a model the provider doesn't serve (404), or the API key is empty, the LLM pass falls back to static-only, with no intent classification, so the verdict over-fires (often 100/DO_NOT_INSTALL). The renderer detects this and prints a degraded-scan banner. Fix the provider/model/key and re-run before trusting the verdict.
- **Exit code 1 is expected** when findings exist. Check that the JSON file was written, not the exit status.
- **Big repos produce big JSON.** The raw report can be hundreds of KB. Never read it whole into context. The renderer reads it; you read the renderer's output. Point the user to the JSON path for full detail.
- **`${CLAUDE_PLUGIN_ROOT}`** is set by Claude Code when the plugin loads. Use it to locate the render script rather than hardcoding a path.
