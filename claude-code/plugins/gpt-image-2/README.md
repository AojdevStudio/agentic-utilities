# gpt-image-2

**Generate images inside Claude Code using your existing ChatGPT subscription — no separate OpenAI API key, no per-image billing.**

This plugin routes image-generation requests through the local `codex` CLI using the session you're already authenticated with for ChatGPT Plus or Pro. You pay nothing extra: image generation is included in your existing plan.

## What it does

- **Text-to-image** — describe what you want, get a PNG. Pass the prompt raw; no style modifiers added without your ask.
- **Image-to-image editing** — attach a reference image and a transformation prompt (e.g., "repaint in watercolor").
- **Style transfer** — use `--ref` to carry composition or color palette from one image into a new rendering.
- **Multi-reference composition** — pass multiple `--ref` flags to blend visual elements from several sources.

All four modes run through the same `scripts/gen.sh` entry point. The script handles session snapshotting, feature-flag activation, and base64 extraction from the Codex session rollout — details that other wrappers routinely get wrong on codex-cli 0.111.0+.

## Trigger phrases

The skill activates when the user explicitly requests GPT Image 2:

- "use GPT Image 2" / "use gpt-image-2" / "use ChatGPT Images 2.0"
- "use Image 2" / "image 2 this"
- attached a reference image and asked to remix / edit / restyle it

The skill does **not** auto-trigger for a generic "generate an image" request — the user must route intentionally. If they do route to this skill, it never silently substitutes DALL·E, Midjourney, an HTML mockup, or a screenshot-based workflow.

## Bundled content

```
skills/gpt-image-2/
├── SKILL.md                 # entry point — trigger conditions, invocation, constraints
└── scripts/
    ├── gen.sh               # main entry point: codex exec wrapper + session diff
    └── extract_image.py     # base64 image extractor from JSONL rollout files
```

## Prerequisites

> **This plugin requires a ChatGPT Plus or Pro subscription and the Codex CLI installed locally.** It does not grant image-generation capability on its own — it exposes the capability you already have.

1. **Codex CLI** — install via `brew install codex` or see [openai/codex on GitHub](https://github.com/openai/codex).
2. **Authenticate** — run `codex login` and sign in with your ChatGPT account.
3. **ChatGPT Plus or Pro** — the `imagegen` tool is gated behind the `image_generation` feature flag, which requires an eligible ChatGPT plan.
4. **python3** on PATH — ships with macOS; on Linux run `apt install python3`.

No OpenAI API key is required or used. This plugin works entirely through the Codex CLI's existing session cookie, the same way you interact with ChatGPT in the browser.

### Alternative: no local install needed

If you don't have Codex CLI or a ChatGPT subscription, you can run GPT Image 2 in the browser via RunComfy (RunComfy account required, separate billing):

- Text-to-image: [runcomfy.com/models/openai/gpt-image-2/text-to-image](https://www.runcomfy.com/models/openai/gpt-image-2/text-to-image)
- Image edit: [runcomfy.com/models/openai/gpt-image-2/edit](https://www.runcomfy.com/models/openai/gpt-image-2/edit)

## User configuration (optional)

Create `.claude/gpt-image-2.local.md` in any project to customize defaults:

```markdown
# gpt-image-2 local config

output_dir: ~/Desktop/generated-images
default_style: photorealistic
default_timeout_sec: 180
```

The skill reads this file when present and falls back to built-in defaults otherwise:

| Setting | Default |
|---------|---------|
| `output_dir` | current working directory |
| `default_style` | none (prompt passed raw) |
| `default_timeout_sec` | 300 |

## What's NOT in this plugin

- **No OpenAI API key support.** This plugin exclusively uses the Codex CLI + ChatGPT session flow. Direct OpenAI API calls (with `OPENAI_API_KEY`) are a different path and not covered here.
- **No DALL·E fallback.** If the user routes to this plugin, it stays on this route or fails with a clear error.
- **No concurrent calls.** The session-file snapshot/diff approach serializes naturally; running two `gen.sh` calls simultaneously may cause incorrect session attribution.
- **No image hosting or upload.** Output is a local file at the path you specify (or `./image-<timestamp>.png` by default).

## License

MIT — see repository LICENSE.
