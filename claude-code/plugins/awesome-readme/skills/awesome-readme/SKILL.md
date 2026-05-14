---
name: awesome-readme
description: "USE WHEN: create readme, write readme, improve readme, rewrite my readme, document a library/CLI/utility, build project docs, get more GitHub stars. Picks Story or Utility mode based on whether there's a real story behind the project."
---

# Awesome README

Create GitHub READMEs that get stars by telling compelling stories — *or* clean utility docs when there's no story to tell.

**Core philosophy:** Match the README to the project. A story-driven framing only works when there's a real story (a pain point, a frustration, an insight). If the project is "just a utility," forcing a fake narrative reads as cheap and hurts trust. Use **Project Mode** to pick the right structure.

## Project Mode (Story vs Utility)

Before drafting, classify the project — either ask the user via `AskUserQuestion`, or detect from context (existing README, CLAUDE.md, scope of the codebase):

| Mode | When to use | Structure emphasis |
|------|-------------|--------------------|
| **Story** (default when a real story exists) | The project solves a real pain point the author personally felt. There's a "before / after" narrative, an insight moment, or a strong opinion behind it. | Hook → Problem → Insight → Solution → Demo → Features → Quick Start → How It Works → **The Story** → Footer |
| **Utility** | The project is a single-purpose tool, library, or scaffold with no narrative reason for existing. Examples: a CLI wrapper, a small dependency, a code-gen template, an internal helper. | Hook (one line) → **Visuals** (diagrams, screenshots, GIFs) → Features → Quick Start → API/Usage → Examples → Footer. **Skip the personal story section entirely.** |
| **Hybrid** | Mostly utility, but a short paragraph of motivation makes it more relatable. | Utility structure + a single 2–3 sentence "Why" callout near the top — never a multi-paragraph narrative arc. |

**Rule:** Never invent a story that isn't there. If the user can't articulate a clear pain/insight when asked, default to **Utility** mode.

When unclear, the Create workflow asks: "Does this project have a real story behind it (a pain you felt, a frustration that drove the build, an opinion you want to defend), or is it a utility/tool with no narrative?"

## Workflow Routing

When executing a workflow, output this notification:

```
Running the **WorkflowName** workflow from the **awesome-readme** skill...
```

| Workflow | Trigger | File |
|----------|---------|------|
| **Create** | "create readme", "write readme", "new readme" | `workflows/Create.md` |
| **Improve** | "improve readme", "make readme better", "more compelling" | `workflows/Improve.md` |
| **Analyze** | "analyze readme", "review readme", "readme feedback" | `workflows/Analyze.md` |

Mode-specific markdown templates live under `references/`:

- `references/story-skeleton.md` — 10-section Story-mode markdown templates + checklist
- `references/utility-skeleton.md` — Utility-mode skeleton + anti-patterns + checklist + visual-generation guidance

Read the relevant reference *only* when the workflow points you there. This keeps context lean — Story-mode runs don't load the Utility skeleton, and vice versa.

## Image Generation

README visuals route through the **gpt-image-2** skill — first-class, default. Image 2 is the strongest current image model for diagrams that contain real labels (~99% text fidelity), and it runs on the user's existing ChatGPT Plus/Pro subscription via the local Codex CLI.

> **Prerequisite — image generation backend.** This plugin assumes one of:
> 1. The **`gpt-image-2`** skill is installed AND the user has the Codex CLI logged in to a ChatGPT Plus/Pro plan with Image 2 entitlement (the default route this plugin documents). See [agentspace-so/agent-skills/gpt-image-2](https://github.com/agentspace-so/agent-skills/tree/main/gpt-image-2) for install instructions.
> 2. A **substitute image-generation skill** the user has wired up (e.g., a Gemini-based generator that accepts a prompt + output path via `GEMINI_API_KEY`). The workflows route by *intent* ("generate a diagram for the README"), so a different generator can stand in if you swap the invocation pattern in `workflows/Create.md` Step 5.
> 3. **Skip image generation entirely** — every workflow respects a "No, text only" answer to its visuals `AskUserQuestion`. The README still ships, just without diagrams.

If neither (1) nor (2) is set up and the user picks "Yes, generate via gpt-image-2", the gen.sh call will exit non-zero (codes 3–7 for missing CLI / not logged in / quota refused). Surface the failing layer in one sentence and ask the user how to proceed. **Do not silently fall back to a weaker model, DALL·E, Midjourney, an HTML mockup, or any "make do" route.**

### How to invoke

When a workflow needs an image, invoke the **gpt-image-2** skill directly:

```bash
bash scripts/gen.sh \
  --prompt "<descriptive prompt for the README element>" \
  --out docs/<descriptive-name>.png
```

For reference-image conditioning (remix an existing logo, restyle a screenshot, etc.):

```bash
bash scripts/gen.sh \
  --prompt "<remix instruction>" \
  --ref /absolute/path/to/reference.png \
  --out docs/<descriptive-name>.png
```

The script lives inside the gpt-image-2 skill's directory; the standard skill loader resolves it. Pass the user's prompt through raw — only polish wording when the user explicitly asks for it. After the image lands, display/attach it in the response; never stop at "saved to docs/x.png".

### Prompt patterns for README elements

Starting points, not mandates. If the user gave you a specific prompt, pass it through raw.

| Element | Prompt shape |
|---------|--------------|
| Hero banner / cover | `"<project> README hero banner. <subject>. Embedded tagline reads: '<tagline>'."` — let Image 2 render the text directly |
| Architecture diagram | `"Architecture diagram for <project>. <A> connects to <B> via <C>. Excalidraw / hand-drawn style. Clear labels on each node."` |
| Workflow illustration | `"Workflow illustration: step 1 <X>, step 2 <Y>, step 3 <Z>. Numbered left-to-right flow with annotated arrows."` |
| Comparison visual | `"Side-by-side comparison: '<A>' vs '<B>'. Two columns, contrasting treatment, clear winner indicated."` |
| Pack / feature icon | `"Square icon for <feature>. Flat design, single accent color, no text."` |

### Hard rules

- Every README image goes through gpt-image-2 (or the user's documented substitute) unless the user explicitly asks for a different route. Do not silently substitute.
- Output paths land in the project's `docs/` directory by default — never the skill's own folder, never a tmp path the user has to fish out later.

## Key Patterns

### Hooks that work

- "Stop [doing painful thing]. Start [doing better thing]."
- "The first [category] that [unique benefit]"
- "[Things] can lie. [Tests/Proof] cannot."

### What NOT to do

- Leading with installation instructions
- Burying the story at the bottom
- Technical jargon before emotional hook
- Missing badges and visual hierarchy
- No call-to-action for stars

## Resources

- [Best-README-Template](https://github.com/othneildrew/Best-README-Template)
- [awesome-readme](https://github.com/matiassingers/awesome-readme)
- [shields.io](https://shields.io) for badges
- [vhs](https://github.com/charmbracelet/vhs) for terminal GIFs
