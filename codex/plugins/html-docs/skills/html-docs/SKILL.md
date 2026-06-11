---
name: html-docs
description: Convert Markdown plans, reports, PR writeups, research docs, or general Markdown files into polished standalone adjacent HTML files. Use when the user asks to port, render, convert, beautify, or make a Markdown doc into HTML.
---

# HTML Docs

Use this skill to turn a Markdown document into a browser-readable HTML artifact. The output must be a single self-contained `.html` file written beside the source document:

```text
docs/VISION.md -> docs/VISION.html
```

## Workflow

1. Resolve the target Markdown file. Use an absolute path when possible.
2. Inspect enough of the source document to understand the doc type: plan, report, PR writeup, research, or general.
3. Run the converter from the installed plugin root:

```bash
PLUGIN_ROOT="${CODEX_PLUGIN_ROOT:-$HOME/plugins/html-docs}"
python3 "$PLUGIN_ROOT/scripts/markdown_to_html.py" /absolute/path/to/doc.md
```

4. If the adjacent HTML file already exists, do not overwrite it unless the user asked to regenerate or replace it. Then run:

```bash
PLUGIN_ROOT="${CODEX_PLUGIN_ROOT:-$HOME/plugins/html-docs}"
python3 "$PLUGIN_ROOT/scripts/markdown_to_html.py" /absolute/path/to/doc.md --force
```

5. Report the generated `.html` path. If a browser tool is available and the user wants visual verification, open the file directly.

## Output Rules

- Write beside the source Markdown file, never into a new output directory.
- Keep the artifact self-contained: no CDN, no remote CSS, no remote JavaScript, no build step.
- Preserve code blocks, tables, task lists, links, headings, and source structure.
- Prefer the converter's deterministic output for repeatable docs. For a bespoke artifact, first create or revise the Markdown into the desired structure, then run the converter.
- For local images, the converter inlines reasonably sized files as data URIs. Remote images are rendered as explicit asset references so the HTML remains dependency-free.

## Useful Commands

Convert with automatic doc-type detection:

```bash
python3 "$HOME/plugins/html-docs/scripts/markdown_to_html.py" ./docs/VISION.md
```

Force a specific doc type:

```bash
python3 "$HOME/plugins/html-docs/scripts/markdown_to_html.py" ./docs/PR.md --doc-type pr --force
```

Verify bundled fixtures:

```bash
python3 "$HOME/plugins/html-docs/scripts/verify_fixtures.py"
```

Fixture examples live at `references/fixtures/*.md`.
