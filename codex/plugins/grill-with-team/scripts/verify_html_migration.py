#!/usr/bin/env python3
"""Verify that markdown context artifacts survived an HTML migration."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


GENERIC_SECTIONS = {
    "glossary",
    "domain map",
    "decision state",
    "decision records",
    "architecture decision records",
}


def normalize(value: str) -> str:
    value = re.sub(r"<[^>]+>", "", value)
    value = re.sub(r"^ADR[- ]?\d+\s*[-:—]\s*", "", value, flags=re.IGNORECASE)
    return re.sub(r"\s+", " ", value).strip()


def slug(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", normalize(value).lower()).strip("-")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def markdown_terms(path: Path) -> set[str]:
    terms: set[str] = set()
    in_glossary = False
    for raw_line in read_text(path).splitlines():
        line = raw_line.strip()
        heading = re.match(r"^(#{2,6})\s+(.+)$", line)
        if heading:
            level = len(heading.group(1))
            title = normalize(heading.group(2))
            title_slug = slug(title)
            if level == 2:
                in_glossary = title_slug == "glossary"
                continue
            if in_glossary and title_slug not in GENERIC_SECTIONS:
                terms.add(title)
            continue

        bullet = re.match(r"^[-*]\s+\*\*([^*]+)\*\*\s*[:.-]", line)
        if bullet:
            terms.add(normalize(bullet.group(1)))
    return terms


def html_terms(path: Path) -> set[str]:
    text = read_text(path)
    terms = set()
    for match in re.finditer(r"<dt[^>]*>(.*?)</dt>", text, re.DOTALL | re.IGNORECASE):
        term = re.sub(r"<span[^>]*>.*?</span>", "", match.group(1), flags=re.DOTALL | re.IGNORECASE)
        terms.add(normalize(term))
    return terms


def adr_index(directory: Path, suffix: str) -> dict[str, str]:
    if not directory.exists():
        return {}
    entries: dict[str, str] = {}
    for path in sorted(directory.glob(f"*.{suffix}")):
        match = re.match(r"^(\d{4})-", path.name)
        if not match:
            continue
        entries[match.group(1)] = adr_title(path)
    return entries


def adr_title(path: Path) -> str:
    text = read_text(path)
    if path.suffix == ".md":
        for line in text.splitlines():
            if line.startswith("# "):
                return normalize(line[2:])
    title = re.search(r"<title[^>]*>(.*?)</title>", text, re.DOTALL | re.IGNORECASE)
    if title:
        return normalize(title.group(1))
    h1 = re.search(r"<h1[^>]*>(.*?)</h1>", text, re.DOTALL | re.IGNORECASE)
    if h1:
        return normalize(h1.group(1))
    return path.stem


def verify_context(markdown_path: Path | None, html_path: Path | None) -> list[str]:
    if not markdown_path or not html_path:
        return []
    if not markdown_path.exists():
        return [f"Missing markdown context: {markdown_path}"]
    if not html_path.exists():
        return [f"Missing HTML context: {html_path}"]

    old_terms = markdown_terms(markdown_path)
    new_terms = html_terms(html_path)
    missing = sorted(old_terms - new_terms)
    extra = sorted(new_terms - old_terms)
    errors = [f"Missing term in HTML: {term}" for term in missing]
    print(f"context terms: markdown={len(old_terms)} html={len(new_terms)}")
    if extra:
        print("extra HTML terms: " + ", ".join(extra))
    return errors


def verify_adrs(markdown_dir: Path | None, html_dir: Path | None) -> list[str]:
    if not markdown_dir or not html_dir:
        return []
    old_adrs = adr_index(markdown_dir, "md")
    new_adrs = adr_index(html_dir, "html")
    errors = []
    for number, title in sorted(old_adrs.items()):
        if number not in new_adrs:
            errors.append(f"Missing ADR-{number} HTML file for: {title}")
    print(f"adrs: markdown={len(old_adrs)} html={len(new_adrs)}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify markdown-to-HTML context migration preservation.")
    parser.add_argument("--context-md", type=Path)
    parser.add_argument("--context-html", type=Path)
    parser.add_argument("--adr-md-dir", type=Path)
    parser.add_argument("--adr-html-dir", type=Path)
    args = parser.parse_args()

    errors = []
    errors.extend(verify_context(args.context_md, args.context_html))
    errors.extend(verify_adrs(args.adr_md_dir, args.adr_html_dir))

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("migration verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
