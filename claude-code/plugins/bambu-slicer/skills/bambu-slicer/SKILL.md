---
name: bambu-slicer
description: "Unified 3D printing pipeline for Bambu Lab printers. USE WHEN user says: \"print\", \"3d print\", \"slice\", \"slice this\", \"make me\", \"design a part\", \"model this\", \"STL\", \"3MF\", \"makerworld\", \"browse models\", \"find me a\", \"printer status\", \"check printer\", \"what's printing\", \"prepare for printing\", \"generate 3MF\", \"fit on one plate\", \"multi-object plate\", \"storage\", \"organizer\", \"mount\", \"holder\", \"bracket\", \"gridfinity\", or provides STL files, photos of things to design, or MakerWorld links. Covers design (OpenSCAD MCP), browse (MakerWorld via agent-browser), slice (OrcaSlicer-backed CLI), multi-plate arrangement, and printer control (Bambu MCP). This is the only skill for anything 3D-printing related."
---

# bambu-slicer — Unified 3D Printing Pipeline

End-to-end workflow for Bambu Lab printers: design custom models, browse MakerWorld for existing ones, slice STLs to print-ready 3MF, arrange multi-object plates, and control the printer — all from the CLI plus surrounding MCPs.

## First-run check

Before slicing anything, verify the user has run plugin setup once:

1. CLI deps installed: `${CLAUDE_PLUGIN_ROOT}/cli/node_modules` exists. If not, instruct: `cd "${CLAUDE_PLUGIN_ROOT}/cli" && bun install`.
2. OrcaSlicer installed at the configured path (default `/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer`, override via `ORCA_CLI_PATH`).
3. Bambu Studio installed (the CLI patches its profile JSONs at slice time — Bambu Studio doesn't need to be running, just installed so the profiles exist on disk).
4. Optional config: `.claude/bambu-slicer.local.md` for printer-specific overrides (machine profile, output dir, printer IP). See plugin README.

If any step fails, surface a clear error before attempting to slice.

## Capabilities & Workflows

### 1. Design Custom Parts (OpenSCAD MCP)

When the user describes something they need or sends a photo, design it with the `openscad` MCP tools.

**Best for:** Functional/geometric parts — storage bins, cable organizers, shelf brackets, phone stands, gridfinity inserts, drawer dividers, wall mounts, enclosures, clips, hooks.

**Not suited for:** Organic/sculpted shapes (use MakerWorld instead).

**Workflow:**
1. Understand dimensions and constraints from description/photo.
2. Write OpenSCAD code using `openscad` MCP → `create_scad_script`.
3. Render preview images → `render_preview`. Show the user for approval.
4. Iterate on design based on feedback.
5. Export STL → `export_stl`.
6. Slice with the bundled CLI (Section 3).

**Design tips:**
- Always ask for or estimate dimensions — a part that doesn't fit is useless.
- Use `$fn=64` or higher for smooth curves.
- Add 0.2 mm tolerance for friction-fit joints.
- Wall thickness minimum 1.2 mm (3 perimeters at a 0.4 mm nozzle).
- Gridfinity standard: 42 mm grid, 7 mm base height.

For tested constants on tray/shelf/riser-style parts, see `references/gotchas.md`.

### 2. Browse & Download from MakerWorld (agent-browser)

When the user wants to find existing models, use the `agent-browser` skill to drive MakerWorld.

**Workflow:**
1. Navigate to `https://makerworld.com/en/search/models?keyword={query}`.
2. Screenshot search results, present options with thumbnails.
3. User picks one → navigate to the model page.
4. Click "Download raw model files" to download STL/STEP files (or grab the source 3MF if posted — see `references/gotchas.md` on remixing).
5. Save to a working directory (e.g., `/tmp/makerworld/`).
6. Slice with the bundled CLI (Section 3).

**Notes:**
- Login required for downloads. Use the user's Bambu account credentials — fetch from their password manager if asked.
- MakerWorld has bot detection — use stealth/realistic browsing patterns.
- Always show the user a screenshot of the model before downloading.
- Check whether the model includes a pre-sliced 3MF for the target printer (saves time).

### 3. Slice STL → Print-Ready 3MF (bundled CLI)

The core slicing engine. Takes STL files and produces print-ready 3MF using OrcaSlicer with patched Bambu Studio profiles.

**CLI usage:**
```bash
# Single file, default settings (0.20 mm Standard, Bambu PLA Basic)
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input model.stl --output model.3mf

# Custom quality
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input model.stl --output model.3mf --quality 0.12

# Custom filament
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input model.stl --output model.3mf --filament "Bambu PETG Basic"

# Multiple objects on one plate (space-separated)
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --input "box.stl cylinder.stl hook.stl" --output plate.3mf

# List available profiles
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" --list-profiles
```

For the full quality + filament tables, see `references/profiles.md`.

**Output location:** Default to `/tmp/` for quick prints, or whatever directory the user has configured (`outputDir` in `.claude/bambu-slicer.local.md`).

### 4. Multi-Object Plate Arrangement

When the user has multiple STLs to print at once, combine them on one plate:

```bash
bun run "${CLAUDE_PLUGIN_ROOT}/cli/cli.ts" \
  --input "part-a.stl part-b.stl part-c.stl" \
  --output combined-plate.3mf
```

The slicer auto-arranges and auto-orients all objects. Use this for batch printing — saves time vs. printing each object separately. See `references/gotchas.md` for bed-margin caveats.

### 5. Printer Control (Bambu MCP)

Use the `bambu-printer` MCP tools for printer interaction. These tools are available after Claude Code restart.

| Task | MCP Tool |
|------|----------|
| Check printer status | `get_printer_status` |
| List printers | `list_printers` |
| Pause current print | `pause_print` |
| Resume paused print | `resume_print` |
| Stop/cancel print | `stop_print` |
| Connect via MQTT (local) | `mqtt_connect` |
| Get real-time status | `printer_get_status` |
| Stream camera | Camera tools |
| Check AMS filament | AMS tools |

**Developer Mode required for local control:** If the user enables Developer Mode on the printer touchscreen (Settings → Network → LAN Only Mode → Developer Mode), full local MQTT control is available including sending prints directly. Without it, cloud-based status monitoring still works.

### 6. Batch Print Night Workflow

When the user wants to maximize a print session:

1. Collect all STLs (designed + downloaded).
2. Group by filament type (don't mix PLA and PETG on one plate).
3. Arrange each group on a plate using multi-object slicing.
4. Slice each plate.
5. Send to printer (or open in Bambu Studio if Developer Mode is off).

## Decision Tree

```
User wants to 3D print something
├── Has specific STL file(s)?       → Slice (Section 3)
├── Wants something custom?         → Design (Section 1)
├── Wants to browse for a model?    → MakerWorld (Section 2)
├── Multiple things to print?       → Multi-plate (Section 4)
├── Asking about printer state?     → Printer Control (Section 5)
└── "Print night" / batch session?  → Batch Workflow (Section 6)
```

## Technical Notes

- **Slicer engine:** OrcaSlicer. Bambu Studio CLI has a known segfault bug with P2S 0.4 mm nozzle profiles ([OrcaSlicer #9636](https://github.com/SoftFever/OrcaSlicer/issues/9636)) — using OrcaSlicer with the bundled profile patcher works reliably.
- **Profile patching:** Machine profiles are auto-patched at slice time. Bambu Studio gcode templates are replaced with OrcaSlicer-compatible versions, AND `inherits` chains are resolved inline so OrcaSlicer can read the profile out-of-tree (parent profiles like `fdm_bbl_3dp_001_common` carry `printable_area` etc., and OrcaSlicer can't follow inherits across a one-off patched file in tmp).
- **Cloud vs. local:** Cloud-based status works without any printer config. Local MQTT control requires Developer Mode + the `bambu-printer` MCP configured with the printer's serial and LAN IP.

## Gotchas, lessons, and tested constants

Read `references/gotchas.md` whenever the user asks for a print — especially a remix from MakerWorld. Covers:

- Bed-margin failures with `--arrange` on near-bed-sized parts.
- Matching the source 3MF's profile when remixing (don't blindly use CLI defaults).
- Two-up plate offers when geometry permits.
- Bambu Studio's harmless version-mapping dialog after opening an OrcaSlicer-produced 3MF.
- Tested fillet/lip/rib constants for tray-style functional parts (shelf risers, drawer organizers).

New 3D-printing lessons go in `references/gotchas.md`. The skill auto-loads it on next session.
