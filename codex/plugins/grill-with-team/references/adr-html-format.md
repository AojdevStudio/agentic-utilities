# ADR (HTML) Format

Architecture Decision Records live in `docs/adr/` as **HTML**, one decision per file, numbered: `0001-slug.html`, `0002-slug.html`. One-per-file is kept (not merged into `CONTEXT.html`) so each decision keeps its own `git` history.

Offer an ADR **only** when all three of Matt's tests hold:
1. **Hard to reverse** — changing your mind later is costly.
2. **Surprising without context** — a future reader will ask "why this way?"
3. **The result of a real trade-off** — genuine alternatives existed.

If any one is missing, skip the ADR.

## Numbering

Scan `docs/adr/` for the highest existing number across `*.html` **and** any leftover `*.md`, then increment. (A freshly migrated repo has only `.html`.)

## Template

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ADR-0007 — Partial cancellation at line-item level</title>
<style>
  /* Reuse the CONTEXT.html design tokens — copy the :root block + base
     body/wrap/masthead/section rules from context-template.html so ADRs
     and CONTEXT share one visual system. Single style block; no inline styles. */
</style>
</head>
<body><div class="wrap">
  <header class="masthead">
    <div class="eyebrow">Architecture Decision Record · ADR-0007</div>
    <h1>Partial cancellation at line-item level</h1>
    <div class="meta">
      <span>status: accepted</span>
      <span class="sep">·</span>
      <span>2026-06-02</span>
    </div>
  </header>

  <section class="sec">
    <div class="sec-head"><span class="num">Context</span></div>
    <p>What forces are at play — the situation that makes a decision necessary.</p>
  </section>

  <section class="sec">
    <div class="sec-head"><span class="num">Decision</span></div>
    <p>The position taken, in active voice. "We will …"</p>
  </section>

  <section class="sec">
    <div class="sec-head"><span class="num">Consequences</span></div>
    <p>What becomes easier and harder as a result. The trade-off, made explicit.</p>
  </section>
</div></body>
</html>
```

## Diff discipline

Same rules as `CONTEXT.html`: pretty-printed, single `<style>` block, surgical edits. Changing an ADR's status (`proposed` → `accepted` → `superseded`) is a one-line edit to the `.meta` status span, not a re-render.
