# Create Workflow

Create a README that fits the project — story-driven when there's a real story, utility-style when there isn't.

## Step 0: Classify Project Mode

**Before anything else, determine the README mode.** See "Project Mode (Story vs Utility)" in `SKILL.md`.

Use `AskUserQuestion` with:
- question: "What kind of README does this project need? Honest answer — most projects don't have a story, and forcing one reads as cheap."
- header: "Project mode"
- options:
  - label: "Story-driven (Recommended only if real)"
    description: "There's a real pain point you felt, a frustration that drove the build, or an opinion you want to defend. Use the full narrative arc."
  - label: "Utility — no story, just useful"
    description: "Single-purpose tool/library with no narrative behind it. Visuals + features + quickstart + API. No fake backstory."
  - label: "Hybrid — utility with a short 'Why'"
    description: "Mostly utility, but a 2–3 sentence motivation paragraph adds context. No multi-paragraph narrative."

**If the user picks Story-driven**, continue with the full Step 4 structure below.
**If Utility**, jump to "Utility Structure" near the bottom of this file and follow that ordering instead.
**If Hybrid**, use Utility Structure plus a single short "Why this exists" callout under the hero.

## Step 1: Gather Context

Before writing anything, understand the project:

1. **Read existing documentation** - CLAUDE.md, package.json, existing README
2. **Identify the tech stack** - What technologies are used?
3. **Find the core problem** - What pain does this solve?
4. **Discover the insight** - What's unique about this approach?

```bash
# Quick context gathering
cat CLAUDE.md 2>/dev/null || cat README.md 2>/dev/null
cat package.json 2>/dev/null | jq '{name, description, keywords}'
ls -la docs/ 2>/dev/null
```

## Step 2: Ask About Visual Content

**REQUIRED: Use AskUserQuestion tool to confirm visuals**

```
Use AskUserQuestion with:
- question: "I'll generate visuals via the gpt-image-2 skill (ChatGPT Images 2.0 — best-in-class for diagrams with real labels). Confirm or skip?"
- header: "Visuals"
- options:
  - label: "Yes, generate via gpt-image-2 (Recommended, default)"
    description: "Architecture diagram, workflow illustration, hero banner — rendered with ~99% text fidelity through your ChatGPT subscription. No per-image billing."
  - label: "No, text only"
    description: "Skip image generation entirely. README ships without diagrams."
  - label: "Use existing images"
    description: "Reference images already in the docs/ folder; I will not generate new ones."
```

**If user selects "Yes":**
- Note which visuals are needed (hero, architecture, workflow, etc.)
- After completing the README draft, invoke the **gpt-image-2** skill for each visual (see Step 5)
- Save outputs into the repo's `docs/` directory with descriptive names

## Step 3: Identify the Emotional Hook

Find the pain point that resonates:

### Hook Formula
```
[Target audience] experiences [painful problem].
Current solutions [fail because X].
This project [unique approach] by [mechanism].
```

### Example Hooks
| Project Type | Hook |
|--------------|------|
| Testing tool | "Stop trusting. Start verifying." |
| DevOps automation | "Your CI pipeline shouldn't wake you at 3am." |
| Database tool | "Migrations that never break production." |
| API framework | "APIs that write their own documentation." |

## Step 4: Structure the README (Story mode)

Follow this 10-section order — story before installation:

1. Hero (hook + badges + tagline)
2. The Problem
3. The Insight
4. The Solution
5. Demo
6. Features / Defense
7. Quick Start
8. How It Works
9. The Story
10. Footer

**Read `references/story-skeleton.md` for the literal markdown template for each section.** That file has the verbatim block to drop in for each heading, plus the Story-mode quality checklist. Don't draft from memory — the templates encode hard-won placement and formatting decisions.

## Step 5: Generate Visuals via gpt-image-2 (If Requested)

If the user said yes in Step 2, invoke the **gpt-image-2** skill — once per visual. Pass the user's intent through raw; only polish wording when the user explicitly asked for "fancy" or "make it look great".

**Pattern:**

```bash
bash scripts/gen.sh \
  --prompt "<README element prompt — see SKILL.md 'Prompt patterns' table>" \
  --out docs/<descriptive-name>.png
```

**Example invocations for a typical Story-mode README:**

```bash
# Hero banner with embedded tagline
bash scripts/gen.sh \
  --prompt "<project> README hero banner. <subject in one phrase>. Embedded tagline reads: '<the tagline>'." \
  --out docs/hero.png

# Architecture diagram
bash scripts/gen.sh \
  --prompt "Architecture diagram for <project>. <ComponentA> connects to <ComponentB> via <ComponentC>. Excalidraw / hand-drawn style. Clear labels on each node." \
  --out docs/architecture.png

# Workflow illustration
bash scripts/gen.sh \
  --prompt "Workflow illustration: step 1 <X>, step 2 <Y>, step 3 <Z>. Numbered left-to-right flow with annotated arrows." \
  --out docs/workflow.png
```

For reference-conditioned remixes (e.g., "restyle this existing logo as ukiyo-e"), add `--ref /absolute/path/to/reference.png`.

After each call:
1. Confirm the output file exists at the `--out` path.
2. Reference it in the README markdown with descriptive alt text.
3. Display/attach the image when reporting back to the user — do not stop at "saved to docs/x.png".

If `gen.sh` exits non-zero (codes 3-7 documented in the gpt-image-2 SKILL.md), name the failing layer in one sentence (e.g., "codex CLI not logged in" or "image_generation feature flag refused") and ask the user how to proceed. Do not silently fall back to a weaker model.

## Step 6: Write and Deliver

1. Write the complete README following the structure
2. Include all images (existing or generated)
3. Replace placeholder `yourname` with actual GitHub username (use `gh api user --jq '.login'`)
4. Validate all links work
5. Present the README to the user

## Quality Checklist (Story mode)

See `references/story-skeleton.md` for the full Story-mode quality checklist.

---

## Utility Structure

Use when Step 0 returned **Utility** (or **Hybrid** + the short "Why" callout). Skip the Problem / Insight / Story sections entirely. Lean on visuals to do the work a story would do in the other mode.

**Read `references/utility-skeleton.md` before drafting.** That file has:

- The full markdown skeleton with literal section headings (Hero → Visuals → Features → Quick Start → API/Usage → Examples → Compatibility → Footer)
- The three permitted visuals headings (`## Visuals` | `## Architecture` | `## How It Works`) and the rule for picking exactly one
- The hard rule banning Story-mode sections
- The "How It Works" placement anti-pattern (it's the visuals heading or it's nothing — never the Story-mode position 8 slot)
- The Hybrid-mode "Why this exists" insertion pattern
- The Utility Mode Quality Checklist
- Visual generation guidance for utility READMEs

The skeleton headings and order are load-bearing — don't draft from memory.
