---
name: skill-inspector
description: Security-inspect an AI agent skill (your own or a third party's) with the skillspector CLI, then deliver a plain-English verdict report in chat (safe / caution / do-not-install, threat breakdown, top findings to check, file hotspots). Use whenever Ossie wants to vet, scan, audit, or sanity-check a skill before installing or trusting it, points at a skill repo/folder/zip and asks "is this safe?", says "inspect this skill", "scan this skill", "check this skill for malware", "skillspector", or shares a GitHub link to a skill/agent and wants to know the risk. Also use to re-read confusing skillspector output.
---

# Skill Inspector

Wraps the `skillspector` security scanner and renders its output as a report a human can actually read. skillspector finds risky patterns in agent skills (data exfiltration, credential access, code execution, prompt injection, persistence, malware signatures) and assigns a 0-100 risk score. Its raw output is hundreds of findings, which is unreadable. This skill collapses that into a verdict, a plain-English threat table, the findings worth checking first, and where they cluster.

## When to reach for which mode

- **Deep (default — Ossie's standing preference):** static + YARA + LLM intent classification. The static-only pass floods the report with false positives (security *vocabulary* in a skill's own docs trips the rules), so a fast scan routinely lands a benign skill in DO_NOT_INSTALL and wastes a round-trip. Deep reads intent and clears those. Always run deep unless explicitly told to go fast. Requires loading the verified LLM config (see Workflow).
- **Fast (only on request):** static + YARA only, `--no-llm`. No keys, deterministic. Use only when Ossie explicitly asks for a quick/static check, or when the LLM provider is genuinely unavailable. Expect false positives; do not deliver a fast-scan verdict as final.

## Workflow

Run the scanner, capture JSON + stderr, then render. Use a slug from the target so parallel scans don't collide. Resolve both paths at runtime (never hardcode a home path):

```bash
# skillspector binary: prefer PATH; otherwise the agentic-utilities repo venv.
SKILLSPEC_DIR="$HOME/Projects/agentic-utilities/skillspector"
SKILLSPEC="$(command -v skillspector || true)"
[ -z "$SKILLSPEC" ] && SKILLSPEC="$SKILLSPEC_DIR/.venv/bin/skillspector"

# This skill's render script lives in scripts/ next to this SKILL.md.
# Set SKILL_DIR to the directory this SKILL.md loaded from.
SKILL_DIR="<dir-containing-this-SKILL.md>"
SLUG=<short-name-of-target>          # e.g. agent-reach

# 1. Load the VERIFIED LLM config (deep scan needs it; the shell's own
#    OPENAI_API_KEY/SKILLSPECTOR_MODEL are often empty or stale and 404).
#    This .env pins gpt-5.4, which is confirmed working on the key. Load only
#    the four LLM vars and strip inline comments — don't blanket-source it.
set -a
eval "$(grep -E '^(SKILLSPECTOR_PROVIDER|SKILLSPECTOR_MODEL|OPENAI_API_KEY|OPENAI_BASE_URL)=' "$SKILLSPEC_DIR/.env" | sed 's/[[:space:]]*#.*//')"
set +a

# 2. Scan (DEEP/LLM default — no --no-llm). Accepts a Git URL, local dir, zip, .md, or file URL.
#    Use 2>| (not 2>) so a re-scan with the same slug isn't blocked by zsh noclobber.
"$SKILLSPEC" scan "<TARGET>" \
  --format json --output "/tmp/skillspector-$SLUG.json" \
  2>| "/tmp/skillspector-$SLUG.stderr"

# 3. Render the readable report
python3 "$SKILL_DIR/scripts/render_report.py" \
  "/tmp/skillspector-$SLUG.json" --scan-log "/tmp/skillspector-$SLUG.stderr"
```

For a **fast/static** scan (only when explicitly requested), add `--no-llm` and skip step 1. The scan exits non-zero whenever findings exist, which is normal, not a failure. Always render the JSON regardless of exit code. Confirm `model=gpt-5.4` in the config echo before trusting a deep verdict — if the key/model didn't load, the scan errors out (good) rather than silently degrading.

## Present the report

The renderer prints chat-ready Markdown. **Paste it straight into the reply.** Don't re-summarize or re-format it; that's the deliverable. Then add at most one line of your own read (for example: "the YARA hits on a CHANGELOG and a PNG are likely false positives, the real concern is the credential-access cluster in `cli.py`").

## How to read the result (so you can explain it)

- **Verdict** is derived from the score band: SAFE (0-14), CAUTION (15-49), DO_NOT_INSTALL (50+). It's a heuristic, not a human judgment.
- **The v2 score is capability-clustered, not a severity sum.** Findings are grouped into capability clusters (one cookie-read capability = one issue, not N hits), each weighted by confidence × LLM-assessed intent (malicious/negligent/benign), combined with diminishing returns so finding *count* no longer pins the score to 100. A **confirmed source→sink exfiltration chain** (taint rules TT3/TT4/TT5) is the categorical DO_NOT_INSTALL trigger — that's the line between "reads X" and "steals X".
- **A 100/100 now almost always means a degraded (static-only) scan**, not a maximally-evil skill. With no LLM, findings get no intent and many static clusters drive the score up. The renderer flags this loudly; treat a degraded verdict as unreliable and re-run with the LLM.
- **Capability ≠ confirmed theft.** The report separates "capability present" (code that *can* read credentials) from "confirmed exfiltration" (a taint chain where data actually leaves). Most credential-access findings are the former — dual-use, often safe to run sandboxed. Judge each cluster; don't treat capability as proven malice.
- **Posture matters.** The "Using it safely" section reframes the same findings for sharing-with-others vs. running-it-yourself-isolated. A capability-only skill is usually fine to run in a container/VM that's denied the real secrets.

## Gotchas

- **Use the pinned model, not the shell's.** The shell often carries an empty `OPENAI_API_KEY` or a stale/uppercase `SKILLSPECTOR_MODEL` that 404s. The `skillspector/.env` pins `gpt-5.4` (verified on the key) — load those four vars as in the Workflow, don't rely on the ambient env. `gpt-5.5` 404s on this key; don't switch to it.
- **Load only the four LLM vars from `.env`, not the whole file.** Blanket-sourcing drags in LangChain/other config and noise. The `grep | sed` one-liner pulls just provider/model/key/base-url and strips inline comments.
- **Exit code 1 is expected** when findings exist. Check that the JSON file was written, not the exit status.
- **Big repos produce big JSON.** The raw report can be hundreds of KB. Never read it whole into context. The renderer reads it; you read the renderer's output. Point the user to the JSON path for full detail.
