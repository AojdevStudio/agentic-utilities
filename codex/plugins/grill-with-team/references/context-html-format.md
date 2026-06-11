# CONTEXT.html Format

`CONTEXT.html` is the single canonical domain artifact: a self-contained HTML page holding the **glossary**, the **visual domain map**, and the **decision state**. It replaces Matt's `CONTEXT.md` within a repo that has run `grill-with-team`. It is a domain document — **never** a spec, scratch pad, or home for implementation detail.

Start from [context-template.html](context-template.html) — copy it verbatim, then fill the three content sections. Do not invent a new design system.

## Structure (locked)

```
<head>
  <style> … shared design system, single block … </style>
</head>
<body><div class="wrap">
  <header class="masthead"> … context name + meta … </header>
  <div class="layout">
    <main class="content">
      <section id="glossary">    … dl.glossary of terms …
      <section id="map">         … SVG entity-relationship diagram …
      <section id="decisions">   … ul.decisions, resolved/open state …
      <section id="adrs">        … links to docs/adr/*.html …
    </main>
    <aside class="toc"> … sticky nav … </aside>
  </div>
</div></body>
```

## Glossary entry (the core unit — edit these surgically)

```html
<div class="term">
  <dt>Order <span class="src">resolved</span></dt>
  <dd>One-paragraph definition in the ubiquitous language.</dd>
  <div class="not"><b>Not</b> a Cart, and <b>not</b> a Fulfillment.</div>
</div>
```

- `dt` — the term. `.src` pill: `resolved` | `open`.
- `dd` — definition. Add a second `dd` for nuance if needed.
- `.not` — the synonyms/confusions this term must NOT drift into. Optional but encouraged.

## Decision-state entry

```html
<li>
  <div class="d-head">
    <span class="d-id">D3</span>
    <span class="d-title">Short decision question or statement</span>
    <span class="pill open">open</span>
  </div>
  <div class="d-body">One or two sentences. Cite ADRs (<a href="docs/adr/0007-…html">ADR-0007</a>) and Explorer findings.</div>
</li>
```

Pills: `resolved` (olive) · `open` (clay) · `assumed` (gray).

## Domain map

An inline `<svg viewBox="0 0 640 240">` with `.ent` rects + `.ent-label` text for entities and `.rel` paths + `.rel-label` for relationships (cardinalities like `places 1..*`). Keep it legible; the Designer pass refines layout. No external diagram libraries.

## Diff discipline (non-negotiable)

1. **One element per line.** Pretty-printed, never minified.
2. **Single `<style>` block.** Never inline `style=` on content elements.
3. **Surgical edits.** To add a term, insert one `<div class="term">` block. To resolve a decision, change the one `<li>`'s pill + body. **Never regenerate the whole file** — that destroys `git diff`/`git blame`, which is the only reason HTML-as-canonical is viable.
