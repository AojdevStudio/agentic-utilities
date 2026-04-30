# bambu-slicer

End-to-end 3D printing workflow for Bambu Lab printers, exposed as a Claude Code skill plus a bundled TypeScript slicing CLI.

The skill auto-activates on print/slice/design/MakerWorld phrases and walks Claude through the full pipeline: design custom parts in OpenSCAD, browse and download from MakerWorld, slice STL → print-ready 3MF with OrcaSlicer, arrange multi-object plates, and drive the printer.

## What it does

- **Design** — write OpenSCAD models via the `openscad` MCP, render previews, export STL.
- **Browse** — drive MakerWorld via the `agent-browser` skill to find existing models.
- **Slice** — run the bundled CLI to convert STL → 3MF using OrcaSlicer with patched Bambu Studio profiles. Multi-object plates auto-arrange.
- **Print control** — read printer status, pause/resume/cancel via the `bambu-printer` MCP.
- **Decision tree + gotchas** — encoded in the skill so Claude picks the right path and avoids known traps (bed-margin failures, MakerWorld remix profile mismatches, BambuStudio version-mapping dialog, etc.).

## Prerequisites

The plugin itself is just a skill + CLI. You bring the rest:

| Dependency | Why | Install |
|------------|-----|---------|
| **Bambu Lab printer** | Default profiles target the P2S; other models work with config overrides (see below). | — |
| **OrcaSlicer** | The slicing engine. Bambu Studio CLI has a known segfault with P2S 0.4 mm nozzle profiles ([#9636](https://github.com/SoftFever/OrcaSlicer/issues/9636)). | `brew install --cask orcaslicer` |
| **Bambu Studio** | Provides the source profile JSON files the CLI patches. | [bambulab.com/download](https://bambulab.com/en/download/studio) |
| **Bun** | Runs the TypeScript CLI without a build step. | `curl -fsSL https://bun.sh/install \| bash` |
| **`openscad` MCP** | For designing custom parts. | Optional — only needed for design workflow. |
| **`bambu-printer` MCP** | For printer control. | Optional — only needed for print/pause/resume. |
| **`agent-browser` skill** | For MakerWorld browsing. | Optional. |

## Setup

After the plugin is installed:

```bash
cd "${CLAUDE_PLUGIN_ROOT}/cli"
bun install
bun run typecheck
bun test
```

The `cli/` directory is a self-contained Bun/TypeScript project. No build step — the CLI runs directly from source.

## Configuration

Drop a `.claude/bambu-slicer.local.md` file in your project (or home dir) to override defaults:

```markdown
---
# Path to OrcaSlicer binary (default: macOS app bundle)
orcaCliPath: /Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer

# Bambu Studio profile root (default: macOS Application Support path)
profileBase: ~/Library/Application Support/BambuStudio/system/BBL

# Machine profile filename (default: "Bambu Lab P2S 0.4 nozzle.json")
machineProfile: Bambu Lab P2S 0.4 nozzle.json

# Default output directory for sliced 3MF files
outputDir: /tmp

# Printer LAN IP (used by bambu-printer MCP; not by this CLI)
printerLanIp: 192.168.1.50
---

Free-form notes Claude can read on first run.
```

The CLI honors these via environment variables (`ORCA_CLI_PATH`, `BAMBU_PROFILE_BASE`, `BAMBU_MACHINE_PROFILE`). Set them in your shell, or have Claude read the `.local.md` file and pass them through.

> **Note**: profiles in `cli/profiles.ts` are P2S-named by default (`@BBL P2S`). Other Bambu printers (X1C, P1S, A1, etc.) use different profile filenames — adapt `BAMBU_MACHINE_PROFILE` and the process/filament maps if needed.

## Quick start

Once installed and configured, just talk to Claude:

- "Design me a phone stand 80×120×100, lip on top." → OpenSCAD MCP
- "Find a gridfinity 4x4 baseplate on MakerWorld." → agent-browser
- "Slice `~/Downloads/model.stl` at 0.12 mm in PLA Matte." → bundled CLI
- "What's the printer doing?" → bambu-printer MCP

The skill triggers automatically on any of those phrases.

## CLI reference

```bash
# Slice a single STL with defaults (0.20 mm Standard, Bambu PLA Basic)
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input model.stl --output model.3mf

# Different quality + filament
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input model.stl --output model.3mf \
  --quality 0.12 --filament "Bambu PETG Basic"

# Multi-object plate (space-separated input files, auto-arranged)
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" \
  --input "box.stl cylinder.stl hook.stl" --output plate.3mf

# List supported profiles
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --list-profiles
```

## Files

```
bambu-slicer/
├── .claude-plugin/plugin.json
├── README.md
├── .gitignore
├── skills/bambu-slicer/
│   ├── SKILL.md                  # Auto-loaded by Claude Code on trigger
│   └── references/
│       ├── profiles.md           # Quality + filament profile tables
│       └── gotchas.md            # Hard-won lessons (bed margins, remix profiles, …)
└── cli/
    ├── cli.ts                    # Bun entry point
    ├── profiles.ts               # Profile resolution
    ├── slicer.ts                 # OrcaSlicer driver + profile patcher
    ├── package.json
    ├── tsconfig.json
    ├── vitest.config.ts
    └── tests/
        ├── profiles.test.ts
        └── slicer.test.ts
```

## License

MIT — see repo root.
