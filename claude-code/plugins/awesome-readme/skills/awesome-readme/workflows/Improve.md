# Improve Workflow

Improve an existing README — story-driven when there's a real story, utility-style when there isn't. Don't force a narrative onto a project that doesn't have one.

## Step 0: Classify Project Mode

Before deciding *how* to improve, decide *what kind of README this should be*. See "Project Mode (Story vs Utility)" in `SKILL.md`.

Use `AskUserQuestion` with:
- question: "What kind of README does this project need? Honest answer — most utilities don't have a story, and forcing one reads as cheap."
- header: "Project mode"
- options:
  - label: "Story-driven (only if real)"
    description: "Real pain point, frustration, or opinion behind the build. Use the full narrative arc — Problem / Insight / Story sections."
  - label: "Utility — no story, just useful"
    description: "Single-purpose tool/library with no narrative. Improve via better visuals, sharper API docs, cleaner Quick Start. Skip story sections entirely."
  - label: "Hybrid — utility with a short 'Why'"
    description: "Mostly utility, but a 2–3 sentence motivation paragraph adds context."

**Story mode** → continue with the existing Step 5 restructure (10-section Story arc).
**Utility mode** → use the *Utility Restructure* below in place of Step 5; remove any existing Story / Insight / Personal Narrative sections rather than rewriting them.
**Hybrid mode** → Utility restructure + a single short "Why this exists" callout under the hero (≤ 3 sentences).

## Step 1: Read the Existing README

```bash
cat README.md
```

Identify current structure and note:
- Where is the installation section? (Before or after the hook?)
- Is there a story? Where is it located?
- Are there visuals?
- What's the emotional hook (if any)?

## Step 2: Diagnose Weaknesses

### Common README Problems

| Problem | Symptom | Fix |
|---------|---------|-----|
| **Installation-first** | Prerequisites/Install is in first 10 lines | Move to after the hook |
| **Buried story** | Personal narrative at bottom or missing | Move to prominent position |
| **No emotional hook** | Starts with "This is a tool that..." | Add provocative tagline |
| **Wall of text** | No tables, no images, no hierarchy | Add visual breaks |
| **Missing CTA** | No star request, no social links | Add footer with CTA |
| **Generic description** | Could describe any project | Find the unique insight |
| **Technical jargon first** | Assumes reader already cares | Earn attention first |

### Score the README (1-10)

| Criterion | Score | Notes |
|-----------|-------|-------|
| Hook strength | | |
| Problem clarity | | |
| Unique insight | | |
| Visual hierarchy | | |
| Story presence | | |
| CTA effectiveness | | |
| **Total** | /60 | |

## Step 3: Ask About Visual Content

**REQUIRED: Use AskUserQuestion tool**

```
Use AskUserQuestion with:
- question: "Your README could use visual content. I'll generate via the gpt-image-2 skill (ChatGPT Images 2.0 — best-in-class for diagrams with real labels). Confirm or skip?"
- header: "Visuals"
- options:
  - label: "Yes, generate via gpt-image-2 (Recommended, default)"
    description: "Architecture diagram, workflow illustration, hero banner — rendered with ~99% text fidelity through your ChatGPT subscription. No per-image billing."
  - label: "Keep existing visuals only"
    description: "Improve the text but don't generate new images."
  - label: "Remove visuals"
    description: "Text-only README (not recommended for stars)."
```

## Step 4: Identify the Core Transformation

Find what makes this project unique:

### Discovery Questions
1. What frustrated the author enough to build this?
2. What's the "aha" moment that makes this approach different?
3. What failure mode does this prevent?
4. Who experiences the most pain from the problem?

### Transformation Formula
```
BEFORE: [What users suffer with current solutions]
INSIGHT: [The breakthrough realization]
AFTER: [How life is better with this project]
```

## Step 5: Restructure

### Story mode

Rearrange sections following the 10-section star-optimized structure: Hero → Problem → Insight → Solution → Demo → Features → Quick Start → How It Works → Story → Footer.

**Read `references/story-skeleton.md` for the literal markdown template per section** and reuse the templates verbatim where existing content fits. Don't restructure from memory — the templates encode placement decisions that matter.

### Utility mode (or Hybrid)

Rearrange to: Hero → Visuals (labeled `## Visuals` / `## Architecture` / `## How It Works`, pick exactly one) → (Hybrid: Why this exists) → Features → Quick Start → API/Usage → Examples → Compatibility → Footer.

**Read `references/utility-skeleton.md`** for the full skeleton, the visuals-heading rule, the Story-mode-section ban, the "How It Works" placement anti-pattern, and the Hybrid "Why" insertion pattern.

When in Utility mode, **delete** any existing `## The Story`, `## The Insight`, `## Why I built this`, or other narrative sections rather than rewriting them. They are the noise to cut, not the signal to amplify.

## Step 6: Rewrite

For each section:
1. Preserve accurate technical information
2. Transform passive voice to active
3. Add visual hierarchy (tables, centered text, dividers)
4. Ensure emotional resonance before technical detail

### Rewriting Principles

| Before | After |
|--------|-------|
| "This tool helps with X" | "Stop suffering through X" |
| "Features include..." | "What makes this different:" |
| "To install, run..." | (Move after hook) |
| "I built this because..." | (Move to prominent Story section) |

## Step 7: Generate Visuals via gpt-image-2 (If Requested)

If the user said yes in Step 3, invoke the **gpt-image-2** skill — once per visual. See SKILL.md "Image Generation" and Create.md Step 5 for the full pattern.

```bash
# Architecture diagram
bash scripts/gen.sh \
  --prompt "Architecture diagram for <project>. <ComponentA> connects to <ComponentB> via <ComponentC>. Excalidraw / hand-drawn style. Clear labels on each node." \
  --out docs/architecture.png

# Workflow illustration
bash scripts/gen.sh \
  --prompt "Workflow illustration: step 1 <X>, step 2 <Y>, step 3 <Z>. Numbered left-to-right flow with annotated arrows." \
  --out docs/workflow.png

# Comparison visual (if applicable)
bash scripts/gen.sh \
  --prompt "Side-by-side comparison: '<A>' vs '<B>'. Two columns, contrasting treatment, clear winner indicated." \
  --out docs/comparison.png
```

Pass the user's prompt through raw — only polish wording when they explicitly asked for it. Reference each output in the README with descriptive alt text. If `gen.sh` fails, name the failing layer in one sentence and ask the user how to proceed; do not silently fall back to a weaker model.

## Step 8: Validate and Deliver

### Checklist
- [ ] Hook is in first 10 lines
- [ ] Installation is NOT in first 50 lines
- [ ] Story is prominent (not buried)
- [ ] At least one visual/diagram
- [ ] Tables used for features
- [ ] Star CTA in footer
- [ ] GitHub username is correct
- [ ] All links work

### Deliver
Present the improved README with:
1. Summary of changes made
2. Before/after comparison of structure
3. Recommendations for additional improvements (GIFs, testimonials, etc.)
