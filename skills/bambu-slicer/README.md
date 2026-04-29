# bambu-slicer

Agent Skill for end-to-end Bambu Lab 3D-printing workflows: design custom parts, browse MakerWorld, slice STL files into print-ready 3MF with OrcaSlicer, arrange multi-object plates, and control printers through a Bambu MCP when available.

## Contents

```text
skills/bambu-slicer/
├── SKILL.md
├── README.md
├── references/
│   ├── gotchas.md
│   └── profiles.md
└── scripts/cli/
    ├── cli.ts
    ├── profiles.ts
    ├── slicer.ts
    ├── package.json
    ├── tsconfig.json
    ├── vitest.config.ts
    └── tests/
```

## Setup

```bash
export BAMBU_SLICER_SKILL_DIR="/path/to/skills/bambu-slicer"
export BAMBU_SLICER_CLI_DIR="$BAMBU_SLICER_SKILL_DIR/scripts/cli"
cd "$BAMBU_SLICER_CLI_DIR"
bun install
bun run typecheck
bun test
```

Prerequisites:

- Bambu Lab printer. Defaults target P2S with 0.4 mm nozzle.
- OrcaSlicer. Default binary: `/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer`.
- Bambu Studio, for local profile JSONs.
- Bun, for the TypeScript CLI.
- Optional: OpenSCAD MCP, Bambu printer MCP, and `agent-browser` skill.

## CLI quick start

```bash
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input model.stl --output model.3mf
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input model.stl --output model.3mf --quality 0.12
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input "a.stl b.stl c.stl" --output plate.3mf
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --list-profiles
```

Runtime overrides:

```bash
export ORCA_CLI_PATH="/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer"
export BAMBU_PROFILE_BASE="$HOME/Library/Application Support/BambuStudio/system/BBL"
export BAMBU_MACHINE_PROFILE="Bambu Lab P2S 0.4 nozzle.json"
```

## Public-sharing safety

This skill is sanitized for open-source sharing. Keep local/private values out of the repo:

- No printer serials, access codes, LAN IPs, Bambu credentials, cloud tokens, Wi-Fi details, local usernames, or private file paths.
- Do not commit `.bambu-slicer.local.md`, `.env`, generated `*.3mf`, `*.gcode`, downloaded private STLs, logs, or screenshots.
- Use placeholders like `<printer-lan-ip>`, `<printer-serial>`, `<access-code>`, and `<output-dir>` in docs.

Secret-scan before publishing:

```bash
rg -n --hidden -i \
  "(api[_-]?key|secret|token|password|credential|serial|access[ _-]?code|lan[ _-]?ip|mqtt|192\\.168|10\\.|172\\.(1[6-9]|2[0-9]|3[0-1]))" \
  skills/bambu-slicer
```
