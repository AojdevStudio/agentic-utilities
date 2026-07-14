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

# skillspector's v2 verdict is a capability-clustered, intent-weighted model:
# findings are grouped into capability clusters (one cookie-read capability is
# one issue, not N), each weighted by confidence x LLM-assessed intent, combined
# with diminishing returns so finding *count* no longer pins the score to 100.
# A confirmed source->sink exfiltration chain (taint) is the categorical
# do-not-install trigger. In practice a 100 now mostly means a DEGRADED scan
# (LLM unavailable -> static-only, no intent to de-escalate); see scan health.
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
    "Prompt Injection": "Text that tries to override the agent's instructions or safety rules — e.g. 'ignore previous instructions'.",
    "Data Flow": "A tracked source→sink data flow (taint). The high-confidence signal that something read actually leaves the skill.",
    "Trigger Abuse": "Activation triggers crafted to fire the skill in contexts the user didn't intend.",
    "MCP Least Privilege": "An MCP tool requesting broader access (wildcards, blanket scopes) than its job needs.",
    "MCP Tool Poisoning": "Hidden instructions in MCP tool/parameter descriptions that an LLM reads and may obey.",
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


def _read_log(stderr_log: str | None) -> str:
    if stderr_log and Path(stderr_log).exists():
        return Path(stderr_log).read_text().lower()
    return ""


def _is_degraded(log: str, meta: dict, risk: dict) -> bool:
    """A deep scan that silently fell back to static-only.

    Detected from the scan log, or inferred when the LLM was requested yet every
    finding carries unknown intent (the LLM never enriched them). Degraded scans
    over-fire: with no intent to de-escalate, many static clusters drive the
    score toward 100, so the verdict is unreliable and usually overstates risk.
    """
    if "llm call failed" in log or "insufficient_quota" in log or "exceeded your current quota" in log:
        return True
    if "model_not_found" in log or "does not exist" in log:
        return True
    if meta.get("llm_requested") and not meta.get("llm_available", True):
        return True
    if meta.get("llm_requested") and risk.get("dominant_intent") == "unknown" and risk.get("score", 0) > 0:
        return True
    return False


def _scan_health(stderr_log: str | None, meta: dict) -> list[str]:
    """Surface things that quietly degrade a scan: LLM fallback, crashes, exec scripts."""
    notes: list[str] = []
    if meta.get("has_executable_scripts"):
        notes.append("📜 Ships **executable scripts** that run code on your machine — isolate accordingly (see below).")
    log = _read_log(stderr_log)
    if "insufficient_quota" in log or "exceeded your current quota" in log:
        notes.append("⚠️ The LLM provider returned **quota exhausted (429)**. Deep analysis **fell back to static-only** — no intent classification, so the verdict overstates risk. Restore quota (or switch provider) and re-run.")
    elif "model_not_found" in log or "does not exist" in log:
        notes.append("⚠️ The configured LLM model was rejected (404). Deep analysis **fell back to static-only**. Fix `SKILLSPECTOR_MODEL`/provider, or run `--no-llm` to make it intentional.")
    elif "llm call failed" in log:
        notes.append("⚠️ An LLM call failed. Some deep analysis fell back to static rules.")
    if "traceback (most recent call last)" in log and "event loop is closed" not in log:
        notes.append("⚠️ skillspector raised an error during the scan, so results may be partial. Re-run with `--verbose` to see why.")
    return notes


def _posture(rec: str, confirmed_exfil: bool, has_scripts: bool) -> list[str]:
    """Deployment-posture guidance: the same findings read differently depending on
    whether you're vetting to share, to use internally, or to run sandboxed."""
    if rec == "SAFE" and not confirmed_exfil:
        return []
    lines: list[str] = []
    if confirmed_exfil:
        lines.append("- **Confirmed exfiltration path** — treat as hostile. Don't run it on any machine with real credentials, and don't share it. If you must inspect behavior, do it in a network-isolated, disposable VM with throwaway accounts only.")
        return lines
    # Capability present, no confirmed theft — the dual-use case. Context decides.
    lines.append("- **Sharing with others:** the conservative verdict above stands — capability like credential/file access is a real liability in someone else's hands.")
    isolate = "no mount of your real credentials, SSH keys, or browser profile; throwaway accounts and scoped API keys only" if has_scripts else "scoped to dummy data"
    lines.append(f"- **Running it yourself:** the risk is *capability*, not confirmed theft. On a secure host, run it isolated — a container/VM with {isolate}. Deny it the real secrets and the flagged capability can't reach anything that matters.")
    lines.append("- **Before trusting it unsandboxed:** read the flagged file(s) below and confirm where the sensitive reads actually go.")
    return lines


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
    confirmed_exfil = bool(risk.get("confirmed_exfiltration"))
    intent = risk.get("dominant_intent", "unknown")
    log = _read_log(stderr_log)
    degraded = _is_degraded(log, meta, risk)
    if not meta.get("llm_requested"):
        mode = "static only"
    elif degraded:
        mode = "static only (LLM requested, unavailable)"
    else:
        mode = "static + LLM"
    files_flagged = len({i["location"].get("file") for i in issues if i.get("location")})

    L: list[str] = []
    L.append(f"# 🔍 Skill Security Report: `{name}`")
    L.append("")
    verdict_icon = "🛑" if rec == "DO_NOT_INSTALL" else ("⚠️" if rec == "CAUTION" else "✅")
    L.append(f"> {verdict_icon} **Verdict: {rec.replace('_', ' ')}.** {VERDICT_MEANING.get(rec, '')}")
    L.append(">")
    L.append(f"> Risk score **{score}/100** ({band} band) · {len(issues)} findings across {files_flagged} files · skillspector {meta.get('skillspector_version', '?')} · {mode}")
    L.append("")
    if degraded:
        L.append("> ⚠️ **Degraded scan — verdict unreliable.** The LLM pass didn't run (see scan health), so findings were never intent-classified. Static-only scans over-fire: the score and verdict likely **overstate** the real risk. Re-run with the LLM to get a trustworthy verdict.")
        L.append("")
    L.append(f"**Capability vs. confirmed theft:** {'🛑 a **confirmed source→sink exfiltration chain** was found — sensitive data actually flows to an external sink, not just gets read.' if confirmed_exfil else 'no confirmed exfiltration chain. Flagged items are *capabilities present* (e.g. code that can read credentials), not proof that data leaves — the difference between “can read X” and “steals X”.'} Likely intent (LLM): **{intent}**.")
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

    # How to use it safely — posture-aware guidance
    posture = _posture(rec, confirmed_exfil, bool(meta.get("has_executable_scripts")))
    if posture:
        L.append("## Using it safely")
        L.append("")
        L.extend(posture)
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
