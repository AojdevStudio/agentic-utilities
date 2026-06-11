#!/usr/bin/env python3
"""Convert Markdown into a polished standalone adjacent HTML document."""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import html
import json
import mimetypes
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable
from urllib.parse import quote, urlparse


DOC_TYPES = {
    "plan": {
        "label": "Implementation plan",
        "accent": "#0F766E",
        "accent_2": "#F97316",
        "surface": "#F7FAF9",
    },
    "report": {
        "label": "Report",
        "accent": "#B45309",
        "accent_2": "#0F766E",
        "surface": "#FAF8F5",
    },
    "pr": {
        "label": "PR writeup",
        "accent": "#4F46E5",
        "accent_2": "#DC2626",
        "surface": "#F8F8FF",
    },
    "research": {
        "label": "Research brief",
        "accent": "#BE123C",
        "accent_2": "#0E7490",
        "surface": "#FFF8FA",
    },
    "general": {
        "label": "HTML document",
        "accent": "#0F766E",
        "accent_2": "#7C3AED",
        "surface": "#F7F8FB",
    },
}


@dataclass
class Heading:
    level: int
    text: str
    slug: str


@dataclass
class RenderedDocument:
    body_html: str
    headings: list[Heading]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert a Markdown document to a self-contained adjacent HTML file."
    )
    parser.add_argument("source", help="Markdown source path")
    parser.add_argument(
        "--output",
        help="Optional explicit output path. Defaults to the source path with .html suffix.",
    )
    parser.add_argument(
        "--doc-type",
        choices=["auto", "plan", "report", "pr", "research", "general"],
        default="auto",
        help="Document treatment to use. Defaults to keyword-based auto detection.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite an existing output file.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Only print errors.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    source = Path(args.source).expanduser().resolve()
    if not source.is_file():
        print(f"Source file not found: {source}", file=sys.stderr)
        return 2
    if source.suffix.lower() not in {".md", ".markdown"}:
        print(f"Source must be a Markdown file: {source}", file=sys.stderr)
        return 2

    output = Path(args.output).expanduser().resolve() if args.output else source.with_suffix(".html")
    if output.exists() and not args.force:
        print(f"Output already exists: {output}. Re-run with --force to overwrite.", file=sys.stderr)
        return 3

    raw = source.read_text(encoding="utf-8")
    html_text = build_html(source, raw, doc_type=args.doc_type)
    output.write_text(html_text, encoding="utf-8")
    if not args.quiet:
        print(output)
    return 0


def build_html(source: Path, raw_markdown: str, *, doc_type: str = "auto") -> str:
    body_markdown, frontmatter = strip_frontmatter(raw_markdown)
    selected_type = detect_doc_type(source, body_markdown) if doc_type == "auto" else doc_type
    theme = DOC_TYPES[selected_type]
    title = extract_title(source, body_markdown, frontmatter)
    subtitle = extract_subtitle(body_markdown)
    rendered = render_markdown(body_markdown, source.parent, skip_first_h1=True)
    highlights = extract_highlights(body_markdown, rendered.headings)
    generated_at = dt.datetime.now(dt.timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M %Z")
    checksum = hashlib.sha256(raw_markdown.encode("utf-8")).hexdigest()[:12]
    word_count = count_words(body_markdown)
    section_count = len([heading for heading in rendered.headings if heading.level <= 2])
    source_name = source.name
    source_json = json.dumps(raw_markdown).replace("</", "<\\/")

    return HTML_TEMPLATE.format(
        title=html.escape(title),
        subtitle=html.escape(subtitle),
        doc_label=html.escape(theme["label"]),
        doc_type=selected_type,
        source_name=html.escape(source_name),
        source_path=html.escape(str(source)),
        generated_at=html.escape(generated_at),
        checksum=html.escape(checksum),
        word_count=f"{word_count:,}",
        section_count=str(section_count),
        accent=theme["accent"],
        accent_2=theme["accent_2"],
        surface=theme["surface"],
        toc=build_toc(rendered.headings),
        highlights=build_highlights(highlights),
        body=rendered.body_html,
        source_json=source_json,
    )


def strip_frontmatter(text: str) -> tuple[str, dict[str, str]]:
    normalized = text.replace("\r\n", "\n")
    if not normalized.startswith("---\n"):
        return normalized, {}
    end = normalized.find("\n---", 4)
    if end == -1:
        return normalized, {}
    frontmatter_text = normalized[4:end]
    body = normalized[normalized.find("\n", end + 4) + 1 :]
    metadata: dict[str, str] = {}
    for line in frontmatter_text.splitlines():
        if ":" not in line or line.startswith((" ", "\t")):
            continue
        key, value = line.split(":", 1)
        metadata[key.strip().lower()] = value.strip().strip("'\"")
    return body, metadata


def detect_doc_type(source: Path, markdown: str) -> str:
    haystack = f"{source.name}\n{markdown[:4000]}".lower()
    scores = {
        "pr": score_terms(
            haystack,
            ["pull request", "pr #", "review focus", "file tour", "diff", "before", "after", "rollback"],
        ),
        "report": score_terms(
            haystack,
            ["weekly", "status", "report", "incident", "postmortem", "post-mortem", "metrics", "shipped"],
        ),
        "research": score_terms(
            haystack,
            ["research", "explainer", "how ", "concept", "glossary", "faq", "learning", "study"],
        ),
        "plan": score_terms(
            haystack,
            ["plan", "roadmap", "milestone", "implementation", "acceptance", "risks", "phase"],
        ),
    }
    best_type, best_score = max(scores.items(), key=lambda item: item[1])
    return best_type if best_score > 0 else "general"


def score_terms(text: str, terms: Iterable[str]) -> int:
    return sum(text.count(term) for term in terms)


def extract_title(source: Path, markdown: str, frontmatter: dict[str, str]) -> str:
    if frontmatter.get("title"):
        return frontmatter["title"]
    for line in markdown.splitlines():
        match = re.match(r"^#\s+(.+?)\s*#*\s*$", line)
        if match:
            return strip_markdown(match.group(1))
    return source.stem.replace("_", " ").replace("-", " ").title()


def extract_subtitle(markdown: str) -> str:
    for paragraph in iter_paragraphs(markdown):
        plain = strip_markdown(paragraph)
        if plain:
            return shorten(plain, 210)
    return "Generated from Markdown as a standalone HTML document."


def extract_highlights(markdown: str, headings: list[Heading]) -> list[str]:
    highlights: list[str] = []
    for line in markdown.splitlines():
        match = re.match(r"^\s*(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?(.+)$", line)
        if not match:
            continue
        text = strip_markdown(match.group(1))
        if text and len(text) > 14:
            highlights.append(shorten(text, 120))
        if len(highlights) >= 4:
            break
    if highlights:
        return highlights
    for heading in headings:
        if heading.level <= 2 and heading.text:
            highlights.append(heading.text)
        if len(highlights) >= 4:
            break
    return highlights or ["Source structure preserved", "Standalone browser document", "Adjacent HTML output"]


def iter_paragraphs(markdown: str) -> Iterable[str]:
    lines: list[str] = []
    for raw_line in markdown.splitlines():
        line = raw_line.rstrip()
        if not line:
            if lines:
                yield " ".join(lines)
                lines = []
            continue
        if re.match(r"^\s*(#{1,6}|[-*+] |\d+[.)] |>|```|\|)", line):
            if lines:
                yield " ".join(lines)
                lines = []
            continue
        lines.append(line)
    if lines:
        yield " ".join(lines)


def render_markdown(markdown: str, base_dir: Path, *, skip_first_h1: bool = False) -> RenderedDocument:
    lines = markdown.replace("\r\n", "\n").split("\n")
    html_blocks: list[str] = []
    headings: list[Heading] = []
    slug_counts: dict[str, int] = {}
    i = 0
    skipped_h1 = False

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        if not stripped:
            i += 1
            continue

        fence = re.match(r"^```([A-Za-z0-9_-]+)?\s*$", stripped)
        if fence:
            language = fence.group(1) or "text"
            code_lines: list[str] = []
            i += 1
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            if i < len(lines):
                i += 1
            html_blocks.append(render_code_block("\n".join(code_lines), language))
            continue

        heading = re.match(r"^(#{1,6})\s+(.+?)\s*#*\s*$", line)
        if heading:
            level = len(heading.group(1))
            text = strip_markdown(heading.group(2))
            slug = unique_slug(text, slug_counts)
            if skip_first_h1 and level == 1 and not skipped_h1:
                skipped_h1 = True
            else:
                headings.append(Heading(level=level, text=text, slug=slug))
                html_blocks.append(
                    f'<h{level} id="{html.escape(slug)}">{render_inline(heading.group(2), base_dir)}</h{level}>'
                )
            i += 1
            continue

        if is_table_start(lines, i):
            table_html, i = render_table(lines, i, base_dir)
            html_blocks.append(table_html)
            continue

        if re.match(r"^\s*>", line):
            quote_lines: list[str] = []
            while i < len(lines) and re.match(r"^\s*>", lines[i]):
                quote_lines.append(re.sub(r"^\s*>\s?", "", lines[i]))
                i += 1
            inner = " ".join(strip_markdown(part) for part in quote_lines).strip()
            html_blocks.append(f"<blockquote>{render_inline(inner, base_dir)}</blockquote>")
            continue

        if re.match(r"^\s*([-*+])\s+", line):
            list_html, i = render_list(lines, i, base_dir, ordered=False)
            html_blocks.append(list_html)
            continue

        if re.match(r"^\s*\d+[.)]\s+", line):
            list_html, i = render_list(lines, i, base_dir, ordered=True)
            html_blocks.append(list_html)
            continue

        if re.match(r"^\s{0,3}(-{3,}|\*{3,}|_{3,})\s*$", line):
            html_blocks.append("<hr>")
            i += 1
            continue

        paragraph_lines = [stripped]
        i += 1
        while i < len(lines) and lines[i].strip() and not is_block_start(lines, i):
            paragraph_lines.append(lines[i].strip())
            i += 1
        paragraph = " ".join(paragraph_lines)
        html_blocks.append(f"<p>{render_inline(paragraph, base_dir)}</p>")

    return RenderedDocument(body_html="\n".join(html_blocks), headings=headings)


def is_block_start(lines: list[str], index: int) -> bool:
    line = lines[index]
    stripped = line.strip()
    if not stripped:
        return True
    if stripped.startswith("```"):
        return True
    if re.match(r"^(#{1,6})\s+", line):
        return True
    if is_table_start(lines, index):
        return True
    if re.match(r"^\s*(>|[-*+] |\d+[.)] )", line):
        return True
    if re.match(r"^\s{0,3}(-{3,}|\*{3,}|_{3,})\s*$", line):
        return True
    return False


def render_code_block(code: str, language: str) -> str:
    lang_label = html.escape(language)
    return (
        '<div class="code-block">'
        f'<div class="code-label">{lang_label}</div>'
        f'<pre><code>{html.escape(code)}</code></pre>'
        "</div>"
    )


def render_list(lines: list[str], index: int, base_dir: Path, *, ordered: bool) -> tuple[str, int]:
    tag = "ol" if ordered else "ul"
    item_re = r"^\s*\d+[.)]\s+(.+)$" if ordered else r"^\s*[-*+]\s+(.+)$"
    items: list[str] = []
    while index < len(lines):
        match = re.match(item_re, lines[index])
        if not match:
            break
        item = match.group(1)
        task = re.match(r"^\[([ xX])\]\s+(.+)$", item)
        if task:
            checked = " checked" if task.group(1).lower() == "x" else ""
            label = render_inline(task.group(2), base_dir)
            items.append(f'<li class="task"><input type="checkbox" disabled{checked}> <span>{label}</span></li>')
        else:
            items.append(f"<li>{render_inline(item, base_dir)}</li>")
        index += 1
    return f"<{tag}>\n" + "\n".join(items) + f"\n</{tag}>", index


def is_table_start(lines: list[str], index: int) -> bool:
    if index + 1 >= len(lines):
        return False
    line = lines[index].strip()
    sep = lines[index + 1].strip()
    if "|" not in line or "|" not in sep:
        return False
    cells = split_table_row(sep)
    return bool(cells) and all(re.match(r"^:?-{3,}:?$", cell.strip()) for cell in cells)


def render_table(lines: list[str], index: int, base_dir: Path) -> tuple[str, int]:
    header = split_table_row(lines[index])
    index += 2
    rows: list[list[str]] = []
    while index < len(lines) and "|" in lines[index] and lines[index].strip():
        rows.append(split_table_row(lines[index]))
        index += 1
    thead = "".join(f"<th>{render_inline(cell.strip(), base_dir)}</th>" for cell in header)
    body_rows = []
    for row in rows:
        cells = row + [""] * max(0, len(header) - len(row))
        body_rows.append("<tr>" + "".join(f"<td>{render_inline(cell.strip(), base_dir)}</td>" for cell in cells[: len(header)]) + "</tr>")
    return (
        '<div class="table-wrap"><table><thead><tr>'
        + thead
        + "</tr></thead><tbody>"
        + "".join(body_rows)
        + "</tbody></table></div>",
        index,
    )


def split_table_row(line: str) -> list[str]:
    stripped = line.strip().strip("|")
    return [cell.strip() for cell in stripped.split("|")]


def render_inline(text: str, base_dir: Path) -> str:
    placeholders: list[str] = []

    def stash(fragment: str) -> str:
        placeholders.append(fragment)
        return f"@@HTMLDOCS{len(placeholders) - 1}@@"

    def image_repl(match: re.Match[str]) -> str:
        alt = match.group(1)
        target = match.group(2).strip()
        return stash(render_image_ref(alt, target, base_dir))

    def link_repl(match: re.Match[str]) -> str:
        label = render_inline(match.group(1), base_dir)
        href = sanitize_href(match.group(2).strip())
        if not href:
            return label
        return stash(f'<a href="{html.escape(href, quote=True)}">{label}</a>')

    text = re.sub(r"`([^`]+)`", lambda m: stash(f"<code>{html.escape(m.group(1))}</code>"), text)
    text = re.sub(r"!\[([^\]]*)\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)", image_repl, text)
    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)", link_repl, text)
    escaped = html.escape(text)
    escaped = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", escaped)
    escaped = re.sub(r"__([^_]+)__", r"<strong>\1</strong>", escaped)
    escaped = re.sub(r"(?<!\*)\*([^*\n]+)\*(?!\*)", r"<em>\1</em>", escaped)
    escaped = re.sub(r"(?<!_)_([^_\n]+)_(?!_)", r"<em>\1</em>", escaped)
    for idx, fragment in enumerate(placeholders):
        escaped = escaped.replace(f"@@HTMLDOCS{idx}@@", fragment)
    return escaped


def render_image_ref(alt: str, target: str, base_dir: Path) -> str:
    parsed = urlparse(target)
    label = html.escape(alt or target)
    if parsed.scheme in {"http", "https"}:
        href = html.escape(target, quote=True)
        return f'<span class="asset-ref">Remote image: <a href="{href}">{label}</a></span>'
    candidate = (base_dir / target).resolve()
    if candidate.is_file() and candidate.stat().st_size <= 1_500_000:
        mime = mimetypes.guess_type(candidate.name)[0] or "application/octet-stream"
        encoded = base64.b64encode(candidate.read_bytes()).decode("ascii")
        return f'<img src="data:{mime};base64,{encoded}" alt="{label}">'
    href = html.escape(target, quote=True)
    return f'<span class="asset-ref">Image reference: <code>{href}</code></span>'


def sanitize_href(target: str) -> str | None:
    parsed = urlparse(target)
    if parsed.scheme and parsed.scheme not in {"http", "https", "mailto"}:
        return None
    return target


def strip_markdown(text: str) -> str:
    text = re.sub(r"!\[([^\]]*)\]\([^)]+\)", r"\1", text)
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    text = re.sub(r"`([^`]+)`", r"\1", text)
    text = re.sub(r"[*_~>#]", "", text)
    text = re.sub(r"\s+", " ", text)
    return text.strip()


def count_words(markdown: str) -> int:
    return len(re.findall(r"\b[\w'-]+\b", strip_markdown(markdown)))


def shorten(text: str, max_len: int) -> str:
    if len(text) <= max_len:
        return text
    return text[: max_len - 1].rsplit(" ", 1)[0].rstrip() + "..."


def slugify(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return slug or "section"


def unique_slug(text: str, counts: dict[str, int]) -> str:
    base = slugify(text)
    counts[base] = counts.get(base, 0) + 1
    return base if counts[base] == 1 else f"{base}-{counts[base]}"


def build_toc(headings: list[Heading]) -> str:
    items = []
    for heading in headings:
        if heading.level > 3:
            continue
        class_name = f"toc-l{heading.level}"
        items.append(
            f'<a class="{class_name}" href="#{html.escape(heading.slug)}">{html.escape(heading.text)}</a>'
        )
    return "\n".join(items) or '<span class="toc-empty">No sections detected</span>'


def build_highlights(highlights: list[str]) -> str:
    return "\n".join(f"<li>{html.escape(item)}</li>" for item in highlights)


HTML_TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  :root {{
    --bg: #f4f6f8;
    --surface: {surface};
    --paper: #ffffff;
    --ink: #17202a;
    --muted: #627084;
    --line: #d7dde6;
    --soft-line: #e8edf3;
    --accent: {accent};
    --accent-2: {accent_2};
    --accent-soft: color-mix(in srgb, var(--accent) 12%, white);
    --shadow: 0 18px 60px rgba(15, 23, 42, 0.10);
    --sans: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    --serif: ui-serif, Georgia, "Times New Roman", serif;
    --mono: ui-monospace, "SF Mono", Menlo, Monaco, Consolas, monospace;
  }}
  * {{ box-sizing: border-box; }}
  html {{ scroll-behavior: smooth; }}
  body {{
    margin: 0;
    background:
      linear-gradient(135deg, rgba(15, 118, 110, 0.08), transparent 34rem),
      linear-gradient(315deg, rgba(79, 70, 229, 0.07), transparent 30rem),
      var(--bg);
    color: var(--ink);
    font: 15px/1.65 var(--sans);
  }}
  button {{ font: inherit; }}
  a {{ color: var(--accent); text-decoration: none; }}
  a:hover {{ text-decoration: underline; }}
  .page {{
    width: min(1180px, calc(100% - 40px));
    margin: 0 auto;
    padding: 46px 0 84px;
  }}
  .hero {{
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 28px;
    align-items: start;
    margin-bottom: 26px;
  }}
  .eyebrow {{
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-bottom: 14px;
    color: var(--muted);
    font: 700 12px/1.2 var(--mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }}
  .type-pill {{
    background: var(--accent);
    color: #fff;
    border-radius: 999px;
    padding: 5px 10px;
    letter-spacing: 0.04em;
  }}
  h1 {{
    margin: 0 0 14px;
    max-width: 860px;
    color: var(--ink);
    font: 650 clamp(2rem, 5vw, 4.5rem)/0.98 var(--serif);
    letter-spacing: 0;
  }}
  .subtitle {{
    max-width: 780px;
    margin: 0;
    color: var(--muted);
    font-size: 18px;
    line-height: 1.55;
  }}
  .actions {{
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    flex-wrap: wrap;
  }}
  .action {{
    border: 1px solid var(--line);
    background: var(--paper);
    color: var(--ink);
    border-radius: 8px;
    padding: 9px 12px;
    cursor: pointer;
    box-shadow: 0 8px 20px rgba(15, 23, 42, 0.06);
  }}
  .action:hover {{ border-color: var(--accent); }}
  .metrics {{
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
    margin: 28px 0 30px;
  }}
  .metric {{
    background: rgba(255, 255, 255, 0.78);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 14px 16px;
    backdrop-filter: blur(10px);
  }}
  .metric .k {{
    color: var(--muted);
    font: 700 11px/1.2 var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 5px;
  }}
  .metric .v {{
    color: var(--ink);
    font-size: 16px;
    font-weight: 750;
    overflow-wrap: anywhere;
  }}
  .layout {{
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 28px;
    align-items: start;
  }}
  .sidebar {{
    position: sticky;
    top: 24px;
    display: grid;
    gap: 16px;
  }}
  .panel {{
    background: rgba(255, 255, 255, 0.84);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 16px;
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.06);
  }}
  .panel h2 {{
    margin: 0 0 12px;
    color: var(--ink);
    font: 750 12px/1.2 var(--mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }}
  .toc a {{
    display: block;
    color: var(--muted);
    padding: 5px 0;
    border-left: 2px solid transparent;
  }}
  .toc a:hover {{
    color: var(--ink);
    border-left-color: var(--accent);
    padding-left: 8px;
    text-decoration: none;
  }}
  .toc .toc-l3 {{ padding-left: 14px; font-size: 13px; }}
  .toc-empty {{ color: var(--muted); font-size: 13px; }}
  .highlights ul {{
    margin: 0;
    padding-left: 18px;
    color: var(--muted);
  }}
  .highlights li {{ margin: 8px 0; }}
  main.content {{
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: clamp(22px, 4vw, 46px);
    box-shadow: var(--shadow);
  }}
  main.content h1,
  main.content h2,
  main.content h3,
  main.content h4 {{
    color: var(--ink);
    line-height: 1.2;
    letter-spacing: 0;
  }}
  main.content h2 {{
    margin: 38px 0 12px;
    padding-top: 22px;
    border-top: 1px solid var(--soft-line);
    font: 650 30px/1.12 var(--serif);
  }}
  main.content h2:first-child {{
    margin-top: 0;
    padding-top: 0;
    border-top: 0;
  }}
  main.content h3 {{ margin: 28px 0 10px; font-size: 20px; }}
  main.content h4 {{ margin: 22px 0 8px; font-size: 16px; }}
  main.content p {{ max-width: 76ch; }}
  main.content ul,
  main.content ol {{ padding-left: 24px; }}
  main.content li {{ margin: 7px 0; }}
  main.content li::marker {{ color: var(--accent); }}
  .task {{
    list-style: none;
    margin-left: -22px;
    display: flex;
    gap: 9px;
    align-items: flex-start;
  }}
  .task input {{ margin-top: 5px; accent-color: var(--accent); }}
  blockquote {{
    margin: 20px 0;
    padding: 14px 18px;
    border-left: 4px solid var(--accent);
    background: var(--accent-soft);
    color: var(--ink);
    border-radius: 0 8px 8px 0;
  }}
  code {{
    font-family: var(--mono);
    font-size: 0.92em;
    background: #edf2f7;
    color: #263241;
    border-radius: 5px;
    padding: 0.15em 0.35em;
  }}
  .code-block {{
    margin: 20px 0;
    border: 1px solid #253044;
    background: #101827;
    border-radius: 8px;
    overflow: hidden;
  }}
  .code-label {{
    color: #9fb0c7;
    background: #172235;
    border-bottom: 1px solid #253044;
    padding: 8px 12px;
    font: 700 11px/1.2 var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }}
  pre {{
    margin: 0;
    overflow-x: auto;
    padding: 16px;
  }}
  pre code {{
    background: transparent;
    color: #e7eefc;
    padding: 0;
    border-radius: 0;
    font-size: 13px;
  }}
  .table-wrap {{
    overflow-x: auto;
    margin: 20px 0;
    border: 1px solid var(--line);
    border-radius: 8px;
  }}
  table {{
    width: 100%;
    border-collapse: collapse;
    min-width: 560px;
  }}
  th, td {{
    border-bottom: 1px solid var(--soft-line);
    padding: 11px 12px;
    text-align: left;
    vertical-align: top;
  }}
  th {{
    background: var(--surface);
    color: var(--ink);
    font: 750 12px/1.35 var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }}
  tr:last-child td {{ border-bottom: 0; }}
  img {{
    max-width: 100%;
    height: auto;
    border-radius: 8px;
    border: 1px solid var(--line);
  }}
  .asset-ref {{
    display: inline-block;
    border: 1px dashed var(--line);
    border-radius: 6px;
    padding: 2px 6px;
    background: #fff;
    color: var(--muted);
    font-size: 0.92em;
  }}
  .source-note {{
    margin-top: 26px;
    color: var(--muted);
    font: 12px/1.5 var(--mono);
  }}
  .toast {{
    position: fixed;
    right: 20px;
    bottom: 20px;
    transform: translateY(20px);
    opacity: 0;
    transition: all 160ms ease;
    background: var(--ink);
    color: #fff;
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 13px;
    pointer-events: none;
  }}
  .toast.on {{ opacity: 1; transform: translateY(0); }}
  @media (max-width: 900px) {{
    .page {{ width: min(100% - 28px, 760px); padding-top: 28px; }}
    .hero {{ grid-template-columns: 1fr; }}
    .actions {{ justify-content: flex-start; }}
    .metrics {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }}
    .layout {{ grid-template-columns: 1fr; }}
    .sidebar {{ position: static; }}
    main.content {{ padding: 22px; }}
  }}
  @media print {{
    body {{ background: #fff; }}
    .actions, .sidebar, .toast {{ display: none !important; }}
    .page {{ width: auto; padding: 0; }}
    .layout {{ display: block; }}
    main.content {{ box-shadow: none; border: 0; padding: 0; }}
  }}
</style>
</head>
<body>
  <div class="page">
    <header class="hero">
      <div>
        <div class="eyebrow"><span class="type-pill">{doc_label}</span><span>Generated from {source_name}</span></div>
        <h1>{title}</h1>
        <p class="subtitle">{subtitle}</p>
      </div>
      <div class="actions" aria-label="Document actions">
        <button class="action" type="button" data-copy-source>Copy markdown</button>
        <button class="action" type="button" onclick="window.print()">Print</button>
      </div>
    </header>

    <section class="metrics" aria-label="Document metrics">
      <div class="metric"><div class="k">Source</div><div class="v">{source_name}</div></div>
      <div class="metric"><div class="k">Words</div><div class="v">{word_count}</div></div>
      <div class="metric"><div class="k">Sections</div><div class="v">{section_count}</div></div>
      <div class="metric"><div class="k">Digest</div><div class="v">{checksum}</div></div>
    </section>

    <div class="layout">
      <aside class="sidebar" aria-label="Document navigation">
        <nav class="panel toc">
          <h2>Contents</h2>
          {toc}
        </nav>
        <section class="panel highlights">
          <h2>Highlights</h2>
          <ul>
            {highlights}
          </ul>
        </section>
      </aside>

      <main class="content doc-type-{doc_type}">
        {body}
        <div class="source-note">
          Generated {generated_at} from {source_path}. Source digest: {checksum}.
        </div>
      </main>
    </div>
  </div>

  <div class="toast" role="status" aria-live="polite" data-toast>Copied</div>
  <script type="application/json" id="source-markdown">{source_json}</script>
  <script>
    const sourceEl = document.getElementById("source-markdown");
    const toast = document.querySelector("[data-toast]");
    const sourceText = sourceEl ? JSON.parse(sourceEl.textContent) : "";
    function showToast(message) {{
      toast.textContent = message;
      toast.classList.add("on");
      window.setTimeout(() => toast.classList.remove("on"), 1400);
    }}
    async function copyText(text) {{
      if (navigator.clipboard && window.isSecureContext) {{
        await navigator.clipboard.writeText(text);
        return;
      }}
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.left = "-9999px";
      document.body.appendChild(area);
      area.focus();
      area.select();
      document.execCommand("copy");
      area.remove();
    }}
    document.querySelector("[data-copy-source]").addEventListener("click", async () => {{
      try {{
        await copyText(sourceText);
        showToast("Markdown copied");
      }} catch (error) {{
        showToast("Copy failed");
      }}
    }});
  </script>
</body>
</html>
"""


if __name__ == "__main__":
    raise SystemExit(main())
