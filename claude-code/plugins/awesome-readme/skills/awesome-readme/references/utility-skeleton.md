# Utility-mode markdown skeleton

Full skeleton, anti-patterns, and quality checklist for Utility-mode (and Hybrid-mode) READMEs. Loaded only when the workflow has classified the project as Utility or Hybrid (via Step 0 of Create.md or Step 5 of Improve.md). Skip this file for Story-mode projects — `references/story-skeleton.md` is what you want.

Utility READMEs replace the narrative arc with visuals. The visuals section does the work a story would do in the other mode — make it the strongest section.

## Hard rule — Story-mode sections are forbidden

Do NOT include any of these in a Utility-mode README: `## The Problem`, `## The Insight`, `## The Solution`, `## The Story Behind X`, `## Why I built this`, or any section whose body reads as personal narrative. These are noise here, not signal.

## Anti-pattern — "How It Works" placement

In Story mode, `## How It Works` is section 8 (deep dive after Quick Start). In Utility mode it does NOT exist as a separate section. If you want a "How It Works" feel, use it as your **Visuals** section heading (it's an acceptable synonym — see template below) and keep it directly under the hero, not in the Story-mode position. Never have BOTH a `## Visuals` section AND a separate `## How It Works` section in Utility mode — pick one heading for that content.

## Visuals heading — pick exactly one

Three options are acceptable for the visuals section heading: `## Visuals` (most generic), `## Architecture` (when the diagram is component-level), or `## How It Works` (when the visual is a data-flow / mechanism diagram). Pick exactly one — whichever names the content most accurately. Never use more than one of these three headings in the same README.

## Skeleton

Use the headings below as **literal section names**. The order is load-bearing — do not move Visuals below Features, do not insert any Story-mode section, do not duplicate the visuals-content slot under a second heading.

````markdown
<div align="center">

# {Project Name}

### **{One literal sentence — what it does, no emotional hook.}**

[![badge1](...)](...)
[![badge2](...)](...)
[![badge3](...)](...)

{One paragraph: who it's for, what it produces, what it does NOT do.}

[**Quick Start**](#quick-start) · [**API**](#api--usage) · [**Examples**](#examples)

</div>

---

## Visuals
<!-- or: ## Architecture, or: ## How It Works — pick the single heading that names the content best -->

![{descriptive alt text}](docs/{name}.png)

{1–3 sentences anchoring the diagram: what the reader is looking at and why it matters. Optionally include a small ASCII fallback for terminal/grep readers.}

---

## Features

| Feature | What it does |
|:--------|:-------------|
| `feature-1` | one-line description |
| `feature-2` | one-line description |

---

## Quick Start

### Install

```bash
{install command}
```

### Run

```{lang}
{minimal usage example — runnable in under 60 seconds}
```

---

## API / Usage

{Full reference: function signatures, CLI commands, or config schema. Users should not need to read the source.}

---

## Examples

{2–3 realistic, copy-pasteable use cases. Real code, not hand-wavy descriptions.}

---

## Compatibility / Requirements

| | Minimum version |
|:--|:--|
| {runtime} | {version} |
| Runtime deps | {none, or list} |

---

## Roadmap

- [x] shipped feature
- [ ] planned feature

## Contributing

{Brief — fork, install, test, PR.}

## License

{License} © [{author}]({link})
````

## Hybrid-mode insertion

For Hybrid (utility + short "Why" callout), insert **between Hero and Visuals**:

```markdown
> **Why this exists.** {2–3 sentence motivation. State the gap or constraint that drove the build. No narrative drama.}
```

Maximum 3 sentences. If it grows longer, the project is actually Story mode and you classified wrong — go back to Step 0.

## Utility Mode Quality Checklist

- [ ] No invented backstory, no fake "frustration that drove the build"
- [ ] Visuals section is **labeled** with one of `## Visuals` / `## Architecture` / `## How It Works`, and sits directly under the hero (not buried after Features)
- [ ] Exactly one of those three headings is used — not all three, not zero
- [ ] At least one diagram, screenshot, or GIF inside that section
- [ ] No `## The Problem`, `## The Insight`, `## The Story Behind X`, `## Why I built this` anywhere in the README
- [ ] Section order matches skeleton: Hero → Visuals → Features → Quick Start → API/Usage → Examples → Compatibility → Footer
- [ ] API/Usage section is complete enough that a user does not need to read the source
- [ ] Quick Start runs end-to-end in under 60 seconds
- [ ] Examples are real and copy-pasteable
- [ ] Hybrid mode "Why" callout is ≤ 3 sentences and lives between Hero and Visuals
- [ ] GitHub username is correct (resolve via `gh api user --jq .login`)

## Visual generation for utilities

Utility-mode READMEs lean **harder** on visuals than Story-mode does — they replace the narrative arc, so they need to do real work. Generate everything through the **gpt-image-2** skill (see SKILL.md "Image Generation").

| Element | gpt-image-2 prompt shape | Notes |
|---------|--------------------------|-------|
| Architecture diagram | `"Architecture diagram for <project>. <A>→<B>→<C>. Excalidraw style. Clear labels."` | Image 2's text fidelity is the win here — labels render legibly |
| Data flow / sequence | *Inline Mermaid in the README* | Skip image generation — Mermaid renders natively on GitHub and diffs cleanly in git |
| Comparison visual | `"Side-by-side comparison: '<A>' vs '<B>'. Two columns, contrasting treatment, clear winner indicated."` | Image 2 handles tabular text better than most image models |
| Annotated screenshot | Capture the screenshot first, then pass as `--ref` to gpt-image-2 with prompt `"Annotated screenshot. Add labeled callouts pointing to: <element 1>, <element 2>. Numbered annotations."` | Reference-conditioned edits preserve the original layout |
| Pack / feature icon | `"Square icon for <feature>. Flat design, single accent color, no text."` | Keep prompts terse for small icons |

**Always** save outputs to `docs/<descriptive-name>.png` and reference them with descriptive alt text in the README.
