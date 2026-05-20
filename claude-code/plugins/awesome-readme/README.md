# awesome-readme

Create GitHub READMEs that earn stars — story-driven when there's a real story, utility-style when there isn't, hybrid when there's a short "Why" worth keeping.

## What it does

The skill auto-activates on requests like "create a readme", "improve my readme", "write docs for this CLI", "rewrite my README to get more stars". It then:

1. **Classifies the project** — Story-driven (real pain point + narrative arc), Utility (single-purpose tool, visuals replace narrative), or Hybrid. Refuses to invent a story that isn't there.
2. **Generates the right structure** — three different `references/` skeletons (story / utility / hybrid) loaded only when relevant, so context stays lean.
3. **Routes visuals through gpt-image-2 by default** — diagrams, hero banners, and architecture art rendered with ~99% text fidelity through the user's existing ChatGPT subscription (no separate API key, no per-image billing).
4. **Three workflows**: `Create` for new READMEs, `Improve` for restructuring existing ones, `Analyze` for scoring without making changes.

## Trigger phrases

- "create a readme for this project"
- "write a readme that gets stars"
- "improve my README, make it more compelling"
- "rewrite my readme"
- "review my readme and tell me how to improve it"
- "document this library / CLI / utility"

## Prerequisite — image generation backend

This plugin generates README visuals through the **`gpt-image-2`** skill, which routes through the Codex CLI to a ChatGPT Plus / Pro subscription. You need **one** of the following set up before the visuals path will work:

| Option | What to install | When to pick this |
|--------|-----------------|-------------------|
| **`gpt-image-2` skill (default)** | `brew install codex` (or see [openai/codex](https://github.com/openai/codex)), then `codex login` with a ChatGPT plan that includes Image 2. Install the [agentspace-so/agent-skills/gpt-image-2](https://github.com/agentspace-so/agent-skills/tree/main/gpt-image-2) skill. | You have a paid ChatGPT plan and want the strongest text-in-diagram fidelity. Recommended. |
| **Substitute image-gen skill (Gemini-based)** | A skill that accepts a prompt + output path and uses `GEMINI_API_KEY` (e.g., a custom Gemini Imagen wrapper). Edit `workflows/Create.md` Step 5 to call that skill instead of `gpt-image-2`. | You don't have a ChatGPT subscription but do have a Gemini API key. |
| **Skip visuals** | Nothing — answer "No, text only" when the workflow asks about visuals. | You only need text-grade README work and don't want to set up an image backend. |

If you don't pick a backend AND don't skip visuals, `gen.sh` exits with codes 3–7. The plugin surfaces the failing layer (CLI missing, not logged in, quota refused) in one sentence and asks how you want to proceed — it never silently falls back to a weaker model.

## Example flow

```
User: "Create a compelling README for my rbp-stack project"
  ↓
Plugin: Asks "Story or Utility mode?" via AskUserQuestion
        → User picks "Story-driven" (real pain point exists)
  ↓
Plugin: Asks "Generate visuals via gpt-image-2?"
        → User picks "Yes (Recommended)"
  ↓
Plugin: Reads the project's CLAUDE.md, package.json, src/ for context
        Identifies the emotional hook
        Reads references/story-skeleton.md for the literal section templates
        Drafts the 10-section README
        Invokes gpt-image-2 for hero banner + architecture diagram + workflow illustration
        Saves images to docs/
        Delivers the final README with images attached
```

## Workflow files

| Workflow | When | File |
|----------|------|------|
| **Create** | New README from scratch | `skills/awesome-readme/workflows/Create.md` |
| **Improve** | Restructure existing README | `skills/awesome-readme/workflows/Improve.md` |
| **Analyze** | Score + recommend without changes | `skills/awesome-readme/workflows/Analyze.md` |

## Reference files (mode-specific, loaded on demand)

| File | Loaded when |
|------|-------------|
| `skills/awesome-readme/references/story-skeleton.md` | Story-mode runs only — 10-section markdown templates + Story-mode quality checklist |
| `skills/awesome-readme/references/utility-skeleton.md` | Utility-mode (and Hybrid) runs only — full skeleton + anti-patterns + Hybrid "Why" insertion + Utility quality checklist + visual-generation guidance |

## License

MIT — see repository LICENSE.
