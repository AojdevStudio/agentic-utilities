#!/usr/bin/env python3
"""Render a skillspector JSON report into a concise, chat-ready Markdown summary.

skillspector emits a flat list of up to hundreds of findings plus a raw 0-100
risk score. Dumping that verbatim is unreadable. This script collapses the noise
into a verdict, a plain-English threat table, the handful of finding-groups worth
checking first, file hotspots, and a scan-health section — everything a human
needs to decide "trust this skill or not" without scrolling through raw JSON.

Results go to stdout (the Markdown report). Diagnostics go to stderr.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

# Severity ordering and the per-finding weight skillspector uses to build the
# 0-100 score (CRITICAL +50, HIGH +25, MEDIUM +10, LOW +5, x1.3 if the skill
# ships executable scripts, capped at 100). We mirror it only to explain the
# score honestly — it saturates fast, so it means "how many red flags", not a
# precise danger dial.
SEV_ORDER = {"CRITICAL": 4, "HIGH": 3, "MEDIUM": 2, "LOW": 1}
SEV_ICON = {"CRITICAL": "🟥", "HIGH": "🟧", "MEDIUM": "🟨", "LOW": "🟦"}

VERDICT_MEANING = {
    "SAFE": "No serious risk patterns found. Reasonable to use after a glance.",
    "CAUTION": "Some risky patterns. Read the findings below before you trust it.",
    "DO_NOT_INSTALL": "Serious risk patterns. Do not install or run this until a human reviews the findings below.",
}

# Plain-English gloss for each threat category, faithful to skillspector's own
# finding explanations. Keeps the report readable for non-security folks.
CATEGORY_GLOSS = {
    "Data Exfiltration": "Code that sends data out, or reads env vars/files that may hold secrets. Could be normal, could be theft.",
    "Dangerous Code Execution": "Runs shell/external commands or eval-style code. Command-injection risk if inputs aren't checked.",
    "Privilege Escalation": "Touches credential files (SSH keys, AWS creds) or asks for more access than it needs.",
    "Excessive Agency": "Does things beyond its stated job (scope creep). An agent acting outside its documented purpose.",
    "Memory Poisoning": "Tries to stuff or manipulate the context window, displacing your instructions and safety rules.",
    "Output Handling": "Uses model/tool output without sanitizing it. Can feed injection into shell, SQL, or HTML downstream.",
    "Rogue Agent": "Sets up persistence (cron, startup scripts, state files) to keep running across sessions.",
    "Supply Chain": "Downloads and runs remote code. Bypasses review and can pull in anything later.",
    "Tool Misuse": "Unsafe defaults (TLS off, no auth, world-writable) that widen the attack surface.",
    "System Prompt Leakage": "Instructions that could expose system prompts or hidden rules to others.",
    "YARA Match": "Matched a known-malware signature (reverse shell, backdoor, info-stealer, C2).",
}


def _load(path: str) -> dict:
    try:
        return json.loads(Path(path).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        sys.stderr.write(f"render_report: cannot read report {path}: {exc}\n")
        sys.exit(2)


def _scan_health(stderr_log: str | None, meta: dict) -> list[str]:
    """Surface things that quietly degrade a scan: LLM fallback, crashes, exec scripts."""
    notes: list[str] = []
    if meta.get("has_executable_scripts"):
        notes.append("📜 Ships **executable scripts**. Risk score was multiplied by 1.3, and scripts run code on your machine.")
    if stderr_log and Path(stderr_log).exists():
        log = Path(stderr_log).read_text().lower()
        if "model_not_found" in log or "does not exist" in log:
            notes.append("⚠️ The configured LLM model was rejected (404). Deep analysis **fell back to static-only**, so findings may be incomplete. Fix `SKILLSPECTOR_MODEL`/provider or run with `--no-llm` to make this intentional.")
        elif "llm call failed" in log:
            notes.append("⚠️ An LLM call failed. Some deep analysis fell back to static rules.")
        if "traceback (most recent call last)" in log and "event loop is closed" not in log:
            notes.append("⚠️ skillspector raised an error during the scan, so results may be partial. Re-run with `--verbose` to see why.")
    return notes


def _render(report: dict, raw_path: str, stderr_log: str | None) -> str:
    skill = report.get("skill", {})
    risk = report.get("risk_assessment", {})
    issues = report.get("issues", [])
    meta = report.get("metadata", {})

    name = skill.get("name") or "unknown"
    if name == "unknown":
        name = Path(skill.get("source", raw_path)).name or "scanned skill"

    rec = risk.get("recommendation", "CAUTION")
    score = risk.get("score", 0)
    band = risk.get("severity", "LOW")
    mode = "static + LLM" if meta.get("llm_requested") else "static only"
    files_flagged = len({i["location"].get("file") for i in issues if i.get("location")})

    L: list[str] = []
    L.append(f"# 🔍 Skill Security Report: `{name}`")
    L.append("")
    verdict_icon = "🛑" if rec == "DO_NOT_INSTALL" else ("⚠️" if rec == "CAUTION" else "✅")
    L.append(f"> {verdict_icon} **Verdict: {rec.replace('_', ' ')}.** {VERDICT_MEANING.get(rec, '')}")
    L.append(">")
    L.append(f"> Risk score **{score}/100** ({band} band) · {len(issues)} findings across {files_flagged} files · skillspector {meta.get('skillspector_version', '?')} · {mode}")
    L.append("")
    if score >= 100:
        L.append("**About the score:** it's capped at 100 and saturates fast (each HIGH adds 25, each CRITICAL 50). A 100 means \"lots of red flags,\" not a precise danger level. Read the findings, don't stop at the number.")
        L.append("")

    # Zero-findings short-circuit: no point rendering empty tables.
    if not issues:
        health = _scan_health(stderr_log, meta)
        L.append("Nothing flagged by the scanner. Note that a clean static scan checks for *known* risky patterns; it isn't proof the skill is benign, just that nothing tripped the rules.")
        if health:
            L.append("")
            L.append("## Scan health")
            L.append("")
            L.extend(health)
        L.append("")
        L.append("---")
        L.append(f"_Raw report: `{raw_path}`_")
        return "\n".join(L)

    # At a glance — severity counts
    sev_counts = Counter(i.get("severity", "LOW") for i in issues)
    L.append("## At a glance")
    L.append("")
    L.append("| Severity | Count |")
    L.append("|---|---|")
    for sev in ("CRITICAL", "HIGH", "MEDIUM", "LOW"):
        if sev_counts.get(sev):
            L.append(f"| {SEV_ICON[sev]} {sev} | {sev_counts[sev]} |")
    L.append("")

    # Threats found — by category, plain English
    cat_counts = Counter(i.get("category", "Other") for i in issues)
    L.append("## What it flagged (plain English)")
    L.append("")
    L.append("| Threat type | Hits | What it means |")
    L.append("|---|---|---|")
    for cat, n in cat_counts.most_common():
        gloss = CATEGORY_GLOSS.get(cat, "(uncategorized)")
        L.append(f"| **{cat}** | {n} | {gloss} |")
    L.append("")

    # Top findings — group by (category, pattern), HIGH+ first, collapse repeats
    groups: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for i in issues:
        groups[(i.get("category", "Other"), i.get("pattern", "unspecified"))].append(i)

    def group_rank(item):
        (cat, pat), members = item
        top_sev = max((SEV_ORDER.get(m.get("severity", "LOW"), 0) for m in members), default=0)
        max_conf = max((m.get("confidence", 0) for m in members), default=0)
        return (top_sev, len(members), max_conf)

    ranked = sorted(groups.items(), key=group_rank, reverse=True)
    # Show groups that are HIGH+; if none, fall back to the top MEDIUM groups.
    high_groups = [g for g in ranked if max(SEV_ORDER.get(m.get("severity", "LOW"), 0) for m in g[1]) >= 3]
    show = high_groups[:10] if high_groups else ranked[:6]

    L.append("## Check these first")
    L.append("")
    if not show:
        L.append("_Nothing high-severity to prioritize._")
    for (cat, pat), members in show:
        top_sev = max(members, key=lambda m: SEV_ORDER.get(m.get("severity", "LOW"), 0)).get("severity", "LOW")
        max_conf = max((m.get("confidence", 0) for m in members), default=0)
        example = members[0].get("location", {})
        loc = f"{example.get('file', '?')}:{example.get('start_line', '?')}"
        where = f"in `{loc}`" + (f" (+{len(members) - 1} more)" if len(members) > 1 else "")
        L.append(f"- {SEV_ICON.get(top_sev, '')} **{cat} · {pat}**: {len(members)} hit(s), confidence up to {max_conf:.0%}. {where}")
        remediation = (members[0].get("remediation") or "").strip().split("\n")[0]
        if remediation:
            L.append(f"  - _Fix:_ {remediation}")
    L.append("")

    # File hotspots
    file_counts = Counter(i["location"].get("file") for i in issues if i.get("location"))
    hot = [(f, n) for f, n in file_counts.most_common(5) if f]
    if hot:
        L.append("## Where the findings cluster")
        L.append("")
        for f, n in hot:
            L.append(f"- `{f}`: {n} finding(s)")
        L.append("")

    # Scan health
    health = _scan_health(stderr_log, meta)
    if health:
        L.append("## Scan health")
        L.append("")
        L.extend(health)
        L.append("")

    L.append("---")
    L.append(f"_Full detail (every finding + remediation): `{raw_path}`_")
    return "\n".join(L)


def main() -> None:
    ap = argparse.ArgumentParser(description="Render a skillspector JSON report as Markdown.")
    ap.add_argument("report", help="Path to skillspector --format json report")
    ap.add_argument("--scan-log", help="Path to captured stderr from the scan (for health warnings)")
    args = ap.parse_args()
    report = _load(args.report)
    sys.stdout.write(_render(report, args.report, args.scan_log) + "\n")


if __name__ == "__main__":
    main()
