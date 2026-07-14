# html-docs

Codex-native plugin for turning Markdown documents into polished, standalone HTML files.

The converter writes next to the source document:

```text
docs/VISION.md -> docs/VISION.html
```

## What it does

- Converts Markdown plans, reports, PR writeups, research notes, and general docs.
- Produces one self-contained HTML file with inline CSS and inline JavaScript.
- Adds a table of contents, summary metrics, highlights, responsive layout, readable tables, code blocks, task lists, and copy/print controls.
- Avoids external CSS, external JavaScript, CDN assets, and build steps.

## Install from this checkout

This repo is the source of truth. Symlink it into the personal Codex plugin directory:

```bash
mkdir -p ~/plugins
ln -sfn "$(pwd)/codex/plugins/html-docs" ~/plugins/html-docs
codex plugin add html-docs@personal
```

## Direct script usage

```bash
python3 ~/plugins/html-docs/scripts/markdown_to_html.py docs/VISION.md
```

Use `--force` to overwrite an existing adjacent HTML file:

```bash
python3 ~/plugins/html-docs/scripts/markdown_to_html.py docs/VISION.md --force
```

## Verify

From this repository, validate the experimental plugin package and its fixtures:

```bash
npm run validate:plugins
python3 codex/plugins/html-docs/scripts/verify_fixtures.py
```

The local Codex validator checks the manifest, declared skills, local references, catalog wiring, junk artifacts, and invokes the fixture verifier. It is repository validation, not evidence from an external official `plugin-validator`. The fixture suite covers implementation plans, weekly/status reports, PR writeups, and research explainers.
