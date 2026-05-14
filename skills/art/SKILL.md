---
name: art
description: "Creates visual assets with high creative range: editorial illustrations, blog headers, technical diagrams, Mermaid diagrams, infographics, thumbnails, wallpapers, icons, comics, annotated screenshots, taxonomies, timelines, maps, comparisons, and dashboards. Use when the user wants an image, diagram, visual concept, prompt, layout, or art direction."
---

# Art

High-creativity visual content skill for Pi.

This is a Pi-native adaptation of your Claude Art workflow library. The original workflow docs, tools, and examples are included in this skill directory so Pi can reuse the same creative system.

## What This Skill Is For

Use this skill when the task involves:
- editorial illustrations
- blog headers and hero images
- technical diagrams
- Mermaid-style diagrams
- comparisons, frameworks, taxonomies, timelines, maps, stats
- thumbnails, icons, wallpapers, comics
- prompt engineering for image generation
- visual direction, composition, and aesthetic decisions

## Pi Adaptation Rules

The imported workflow files contain some legacy Claude/PAI-only instructions. Translate them like this:

- Ignore all voice notification steps, `curl ${PAI_NOTIFY_URL:-}`, and similar harness-specific commands.
- Ignore slash commands like `/cse`. When a workflow says to run story explanation or CSE-24, do the equivalent analysis yourself in-chat: produce a deep 24-point narrative or structural breakdown before drafting the visual.
- Ignore references to `AskUserQuestion`, `TaskGet`, `TaskUpdate`, `${PROJECTS_DIR}`, and similar unavailable tooling.
- Treat `${PAI_DIR}` references as this skill directory: `~/.pi/agent/skills/art/`.
- For generation, prefer the local tool at `Tools/Generate.ts`.
- Always output generated images to `~/Downloads/` first for review.
- If direct image generation is unavailable, still complete the creative work by producing:
  1. a strong concept,
  2. a polished production-ready prompt,
  3. model/size/aspect-ratio recommendations,
  4. file naming and placement guidance,
  5. Mermaid/SVG/ASCII fallback where appropriate.

## Available Local Assets

- Workflows: `Workflows/*.md`
- Image generation CLI: `Tools/Generate.ts`
- Prompt tooling: `Tools/GeneratePrompt.ts`
- Midjourney integration: `Tools/GenerateMidjourneyImage.ts` (only if env keys exist)
- Examples: `Examples/*.png`
- Artist context: `references/ArtistContext.md`

## Backends

This adapted skill can use these image backends when credentials are present:
- `nano-banana-pro` via Google
- `flux` via Replicate
- `nano-banana` via Replicate
- `gpt-image-1` if OpenAI image credentials exist
- background removal via remove.bg if configured

Prefer this order:
1. `nano-banana-pro` for text-heavy diagrams, editorial illustration, consistency, and best all-around quality
2. `flux` for stylistic variety
3. `nano-banana` for faster drafts
4. `gpt-image-1` for alternate diagram/text rendering when available

## Output Rule

All generated images go to `~/Downloads/` first.

Only after the user approves should they be copied into project directories.

## Workflow Routing

Route requests like this and load the matching file with `read` before doing the work:

| Request | Workflow |
|---|---|
| blog header, editorial illustration, essay art | `Workflows/Essay.md` |
| technical or architecture diagram | `Workflows/TechnicalDiagrams.md` |
| Mermaid diagram, flowchart, sequence diagram | `Workflows/Mermaid.md` |
| comparison, X vs Y | `Workflows/Comparisons.md` |
| framework, 2x2, quadrant | `Workflows/Frameworks.md` |
| taxonomy, classification | `Workflows/Taxonomies.md` |
| timeline, chronology | `Workflows/Timelines.md` |
| map, territory, landscape | `Workflows/Maps.md` |
| stat card, metric visual | `Workflows/Stats.md` |
| data visualization, dashboard | `Workflows/D3Dashboards.md` or `Workflows/Visualize.md` |
| annotated screenshot | `Workflows/AnnotatedScreenshots.md` |
| recipe card, step-by-step | `Workflows/RecipeCards.md` |
| aphorism, quote card | `Workflows/Aphorisms.md` |
| comic, sequential panels | `Workflows/Comics.md` |
| thumbnail | `Workflows/YouTubeThumbnailChecklist.md` or `Workflows/AdHocYouTubeThumbnail.md` |
| icon | `Workflows/CreatePAIPackIcon.md` |
| wallpaper | `Workflows/EmbossedLogoWallpaper.md` or `Workflows/ULWallpaper.md` |
| background removal | `Workflows/RemoveBackground.md` |
| not sure which visual format fits | `Workflows/Visualize.md` |

## Operating Pattern

1. Identify the visual job.
2. Load the matching workflow file.
3. Do the deep concept analysis the workflow asks for, translating any legacy commands into normal reasoning.
4. Inspect relevant local files, content, screenshots, or examples.
5. Produce either:
   - the final asset, or
   - the strongest generation prompt + execution command + output path, or
   - a Mermaid/SVG/ASCII fallback if image generation is not the right tool.
6. Validate the work against the workflow checklist before stopping.

## Quality Bar

Aim for publication-quality visuals.

Default standard:
- strong concept
- clear composition
- deliberate typography guidance
- controlled color use
- emotionally appropriate style
- no generic filler aesthetics
- technically correct diagrams
- outputs named clearly in `~/Downloads/`

## Example Commands

```bash
cd ~/.pi/agent/skills/art/Tools
bun run Generate.ts \
  --model nano-banana-pro \
  --prompt "[PROMPT]" \
  --size 2K \
  --aspect-ratio 16:9 \
  --output ~/Downloads/visual-concept.png
```

```bash
cd ~/.pi/agent/skills/art/Tools
bun run Generate.ts \
  --model nano-banana-pro \
  --prompt "[PROMPT]" \
  --size 2K \
  --aspect-ratio 1:1 \
  --thumbnail \
  --output ~/Downloads/blog-header.png
```

## Notes

- The workflow docs are intentionally rich and opinionated. Use them as creative operating manuals, not as literal harness-specific scripts.
- Read `references/ArtistContext.md` when extra aesthetic range or image-prompt quality is needed.
