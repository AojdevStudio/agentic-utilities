---
name: skill-inspector
description: Security-inspect an AI agent skill (your own or a third party's) with the skillspector CLI, then deliver a plain-English verdict report in chat (safe / caution / do-not-install, threat breakdown, top findings to check, file hotspots). Use whenever Ossie wants to vet, scan, audit, or sanity-check a skill before installing or trusting it, points at a skill repo/folder/zip and asks "is this safe?", says "inspect this skill", "scan this skill", "check this skill for malware", "skillspector", or shares a GitHub link to a skill/agent and wants to know the risk. Also use to re-read confusing skillspector output.
---

# Skill Inspector

Wraps the `skillspector` security scanner and renders its output as a report a human can actually read. skillspector finds risky patterns in agent skills (data exfiltration, credential access, code execution, prompt injection, persistence, malware signatures) and assigns a 0-100 risk score. Its raw output is hundreds of findings, which is unreadable. This skill collapses that into a verdict, a plain-English threat table, the findings worth checking first, and where they cluster.

## When to reach for which mode

- **Fast (default):** static + YARA rules only. No API keys, no cost, deterministic, always works. Use this for quick "is this safe?" checks. Pass `--no-llm`.
- **Deep:** adds LLM analysis (intent classification, subtler findings). Needs a *working* provider + model in the environment. Slower, costs tokens. Only use when the fast scan is ambiguous and the extra depth is worth it.

## Workflow

Run the scanner, capture JSON + stderr, then render. Use a slug from the target so parallel scans don't collide. Resolve both paths at runtime (never hardcode a home path):

```bash
# skillspector binary: prefer PATH; otherwise the agentic-utilities repo venv.
SKILLSPEC="$(command -v skillspector || true)"
[ -z "$SKILLSPEC" ] && SKILLSPEC="$HOME/Projects/agentic-utilities/skillspector/.venv/bin/skillspector"

# This skill's render script lives in scripts/ next to this SKILL.md.
# Set SKILL_DIR to the directory this SKILL.md loaded from.
SKILL_DIR="<dir-containing-this-SKILL.md>"
SLUG=<short-name-of-target>          # e.g. agent-reach

# 1. Scan (fast/static default). Accepts a Git URL, local dir, zip, .md, or file URL.
#    Use 2>| (not 2>) so a re-scan with the same slug isn't blocked by zsh noclobber.
"$SKILLSPEC" scan "<TARGET>" --no-llm \
  --format json --output "/tmp/skillspector-$SLUG.json" \
  2>| "/tmp/skillspector-$SLUG.stderr"

# 2. Render the readable report
python3 "$SKILL_DIR/scripts/render_report.py" \
  "/tmp/skillspector-$SLUG.json" --scan-log "/tmp/skillspector-$SLUG.stderr"
```

For a **deep** scan, drop `--no-llm`. The scan exits non-zero whenever findings exist, which is normal, not a failure. Always render the JSON regardless of exit code.

## Present the report

The renderer prints chat-ready Markdown. **Paste it straight into the reply.** Don't re-summarize or re-format it; that's the deliverable. Then add at most one line of your own read (for example: "the YARA hits on a CHANGELOG and a PNG are likely false positives, the real concern is the credential-access cluster in `cli.py`").

## How to read the result (so you can explain it)

- **Verdict** is derived purely from the score band: SAFE (score 0-20), CAUTION (21-50), DO_NOT_INSTALL (51+). It's a heuristic, not a human judgment.
- **The score saturates.** Each HIGH adds 25, each CRITICAL 50, capped at 100, then multiplied by 1.3 if the skill ships executable scripts. Two HIGH findings already max a small skill into the danger band. So 100/100 means "many red flags," not "maximally evil." The *findings* carry the signal, not the number. The report says this; reinforce it.
- **Findings can be false positives.** A YARA `info_stealer` hit on a binary asset or a changelog, or "Env Variable Harvesting" inside a test file, is often benign. Static analysis flags patterns, not proven intent. Judge each cluster; don't treat every finding as a confirmed threat.
- **Scan health matters.** If the report's "Scan health" section says the LLM fell back, a deep scan silently degraded to static. Say so, and offer to re-run once the provider is fixed.

## Gotchas

- **Broken LLM config is common.** If the environment sets `SKILLSPECTOR_MODEL` to a model the provider doesn't serve (a model that 404s), a non-`--no-llm` scan still produces a report but quietly falls back to static. The renderer detects this from stderr and flags it. Prefer `--no-llm` unless deep analysis is specifically wanted.
- **Don't source a project `.env` just to scan.** The fast path needs no credentials. Sourcing a `.env` can drag in a broken `SKILLSPECTOR_MODEL`/provider and degrade the scan. Run clean.
- **Exit code 1 is expected** when findings exist. Check that the JSON file was written, not the exit status.
- **Big repos produce big JSON.** The raw report can be hundreds of KB. Never read it whole into context. The renderer reads it; you read the renderer's output. Point the user to the JSON path for full detail.
