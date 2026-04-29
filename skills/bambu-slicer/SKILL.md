---
name: bambu-slicer
description: "Unified 3D printing pipeline for Bambu Lab printers. USE WHEN user says: print, 3d print, slice, slice this, make me, design a part, model this, STL, 3MF, MakerWorld, browse models, find me a model, printer status, check printer, what's printing, prepare for printing, generate 3MF, fit on one plate, multi-object plate, storage, organizer, mount, holder, bracket, gridfinity, or provides STL files, photos of things to design, or MakerWorld links. Covers design with OpenSCAD MCP, browsing MakerWorld with agent-browser, slicing STL to 3MF with the bundled OrcaSlicer-backed CLI, multi-plate arrangement, and printer control with Bambu MCP. This is the only skill for 3D-printing related work."
---

# bambu-slicer — Unified 3D Printing Pipeline

End-to-end workflow for Bambu Lab printers: design custom models, browse MakerWorld for existing ones, slice STLs to print-ready 3MF, arrange multi-object plates, and control the printer — all from Agent Skills plus surrounding MCPs.

## Security and privacy guardrails

This skill is intended for public sharing. Keep it clean:

- Never write printer serial numbers, access codes, Bambu account credentials, cloud tokens, LAN IPs, Wi-Fi details, local usernames, private file paths, or customer/private model names into `SKILL.md`, `references/`, package manifests, READMEs, or committed config.
- Treat `.bambu-slicer.local.md`, `.env`, downloaded private STLs, generated 3MF/G-code, printer logs, screenshots, and MakerWorld session data as local/private.
- If MakerWorld login is required, use an existing browser session or explicit user-approved password-manager flow. Do not ask the user to paste credentials into chat, and do not store credentials in files.
- Use placeholders in public docs: `<printer-lan-ip>`, `<printer-serial>`, `<access-code>`, `<output-dir>`.
- Before publishing changes, run a secret scan over `skills/bambu-slicer` and confirm only placeholders/defaults are present.

## First-run check

Before slicing anything, verify setup once:

1. Locate this skill directory. In this repo it is `skills/bambu-slicer`; when installed globally it may be `~/.pi/agent/skills/bambu-slicer` or another Agent Skills path. If needed, set:

   ```bash
   export BAMBU_SLICER_SKILL_DIR="/path/to/skills/bambu-slicer"
   export BAMBU_SLICER_CLI_DIR="$BAMBU_SLICER_SKILL_DIR/scripts/cli"
   ```

2. CLI deps installed: `$BAMBU_SLICER_CLI_DIR/node_modules` exists. If not:

   ```bash
   cd "$BAMBU_SLICER_CLI_DIR" && bun install
   ```

3. OrcaSlicer installed at the configured path. Default: `/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer`; override with `ORCA_CLI_PATH`.
4. Bambu Studio installed. The CLI patches its profile JSONs at slice time; Bambu Studio does not need to be running, but profiles must exist on disk.
5. Optional local config exists outside committed files, for example `.bambu-slicer.local.md` or `~/.config/bambu-slicer/local.md`, with placeholders resolved by the user. The CLI itself reads environment variables: `ORCA_CLI_PATH`, `BAMBU_PROFILE_BASE`, and `BAMBU_MACHINE_PROFILE`.

If any step fails, surface a clear error before attempting to slice.

## Capabilities and workflows

### 1. Design custom parts with OpenSCAD MCP

When the user describes something they need or sends a photo, design it with the `openscad` MCP tools.

**Best for:** Functional/geometric parts — storage bins, cable organizers, shelf brackets, phone stands, gridfinity inserts, drawer dividers, wall mounts, enclosures, clips, hooks.

**Not suited for:** Organic/sculpted shapes. Use MakerWorld instead.

Workflow:

1. Understand dimensions and constraints from description/photo.
2. Write OpenSCAD code using `openscad` MCP → `create_scad_script`.
3. Render preview images → `render_preview`. Show the user for approval.
4. Iterate on design based on feedback.
5. Export STL → `export_stl`.
6. Slice with the bundled CLI.

Design tips:

- Always ask for or estimate dimensions. A part that does not fit is useless.
- Use `$fn=64` or higher for smooth curves.
- Add 0.2 mm tolerance for friction-fit joints.
- Wall thickness minimum: 1.2 mm, equivalent to 3 perimeters at a 0.4 mm nozzle.
- Gridfinity standard: 42 mm grid, 7 mm base height.

For tested constants on tray/shelf/riser-style parts, read `references/gotchas.md`.

### 2. Browse and download from MakerWorld with agent-browser

When the user wants to find existing models, use the `agent-browser` skill to drive MakerWorld.

Workflow:

1. Navigate to `https://makerworld.com/en/search/models?keyword={query}`.
2. Screenshot search results and present options with thumbnails.
3. User picks one → navigate to the model page.
4. Click "Download raw model files" to download STL/STEP files, or use the source 3MF if posted. See `references/gotchas.md` before remixing.
5. Save to a local working directory such as `/tmp/makerworld/`.
6. Slice with the bundled CLI.

Notes:

- Login may be required for downloads. Use only existing authenticated sessions or an explicit user-approved password-manager flow.
- MakerWorld has bot detection. Use realistic browsing patterns.
- Always show the user a screenshot of the model before downloading.
- Check whether the model includes a pre-sliced 3MF for the target printer. It may save time.

### 3. Slice STL to print-ready 3MF with bundled CLI

The core slicing engine takes STL files and produces print-ready 3MF using OrcaSlicer with patched Bambu Studio profiles.

Set the CLI path once per shell/session:

```bash
export BAMBU_SLICER_CLI_DIR="${BAMBU_SLICER_CLI_DIR:-$BAMBU_SLICER_SKILL_DIR/scripts/cli}"
```

CLI usage:

```bash
# Single file, default settings: 0.20 mm Standard, Bambu PLA Basic
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input model.stl --output model.3mf

# Custom quality
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input model.stl --output model.3mf --quality 0.12

# Custom filament
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input model.stl --output model.3mf --filament "Bambu PETG Basic"

# Multiple objects on one plate, space-separated
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input "box.stl cylinder.stl hook.stl" --output plate.3mf

# List available profiles
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --list-profiles
```

For the full quality and filament tables, read `references/profiles.md`.

Output location: default to `/tmp/` for quick prints, or the user's configured output directory. Do not write generated 3MF/G-code into committed skill files.

### 4. Multi-object plate arrangement

When the user has multiple STLs to print at once, combine them on one plate:

```bash
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" \
  --input "part-a.stl part-b.stl part-c.stl" \
  --output combined-plate.3mf
```

The slicer auto-arranges and auto-orients all objects. Use this for batch printing. See `references/gotchas.md` for bed-margin caveats.

### 5. Printer control with Bambu MCP

Use `bambu-printer` MCP tools for printer interaction when available.

| Task | MCP tool |
| --- | --- |
| Check printer status | `get_printer_status` |
| List printers | `list_printers` |
| Pause current print | `pause_print` |
| Resume paused print | `resume_print` |
| Stop/cancel print | `stop_print` |
| Connect via MQTT locally | `mqtt_connect` |
| Get real-time status | `printer_get_status` |
| Stream camera | Camera tools |
| Check AMS filament | AMS tools |

Developer Mode is required for full local control. If the user enables Developer Mode on the printer touchscreen, local MQTT control is available. Without it, cloud-based status monitoring may still work.

Do not expose or commit printer serial numbers, LAN IPs, access codes, or MQTT credentials.

### 6. Batch print night workflow

When the user wants to maximize a print session:

1. Collect all STLs, both designed and downloaded.
2. Group by filament type. Do not mix PLA and PETG on one plate.
3. Arrange each group on a plate using multi-object slicing.
4. Slice each plate.
5. Send to printer via approved printer-control flow, or open in Bambu Studio if local Developer Mode is off.

## Decision tree

```text
User wants to 3D print something
├── Has specific STL file(s)?       → Slice
├── Wants something custom?         → Design with OpenSCAD
├── Wants to browse for a model?    → MakerWorld with agent-browser
├── Multiple things to print?       → Multi-object plate
├── Asking about printer state?     → Bambu MCP printer control
└── "Print night" / batch session?  → Batch workflow
```

## Technical notes

- Slicer engine: OrcaSlicer. Bambu Studio CLI has a known segfault bug with P2S 0.4 mm nozzle profiles ([OrcaSlicer #9636](https://github.com/SoftFever/OrcaSlicer/issues/9636)), so this skill uses OrcaSlicer with the bundled profile patcher.
- Profile patching: machine profiles are auto-patched at slice time. Bambu Studio gcode templates are replaced with OrcaSlicer-compatible versions, and `inherits` chains are resolved inline so OrcaSlicer can read the profile out-of-tree.
- Cloud vs. local: cloud-based status may work without printer config. Local MQTT control requires Developer Mode plus a configured `bambu-printer` MCP.

## Gotchas, lessons, and tested constants

Read `references/gotchas.md` whenever the user asks for a print, especially a MakerWorld remix. It covers:

- Bed-margin failures with `--arrange` on near-bed-sized parts.
- Matching the source 3MF's profile when remixing.
- Two-up plate offers when geometry permits.
- Bambu Studio's harmless version-mapping dialog after opening an OrcaSlicer-produced 3MF.
- Tested fillet/lip/rib constants for tray-style functional parts.

New 3D-printing lessons go in `references/gotchas.md`. Keep entries tight: failure mode → root cause → fix → why it matters.
