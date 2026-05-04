# Gotchas & Lessons

Hard-won lessons from real prints. Read this whenever the user asks for a print — especially a MakerWorld remix.

## Bed margins for `--arrange`

OrcaSlicer's `--arrange` step on a 256×256 mm bed needs at least ~10 mm of bed margin around a part to succeed. A 254 mm-wide part on a 256 mm bed will fail with `"Nothing to be sliced ... no object is fully inside the print volume before apply"` even though the dimensions technically fit.

**Cap individual-part width at ~240 mm** unless the user explicitly waives auto-arrange. Smaller parts can pack tighter; the issue is just margin around a single near-bed-sized object.

For non-256 mm beds (A1, A1 mini, X1C, etc.), apply the same ~6% margin rule of thumb.

## Match the source's profile when remixing a MakerWorld 3MF

When the user hands you a source 3MF from MakerWorld and asks you to modify the model, **default to the source 3MF's slicing profile** (infill density, wall count, layer height) — not the CLI's defaults. The CLI defaults to 0.2 mm Standard / 15 % infill / Bambu PLA Basic, which is fine for most prints but is wrong for shell-style decorative pieces.

How to read the source profile:

1. Unzip the source `.3mf` and look at `Metadata/project_settings.config` for `sparse_infill_density`, `wall_loops`, `layer_height`, `sparse_infill_pattern`, `top_shell_layers`, `bottom_shell_layers`.
2. Or check the model description/title — MakerWorld designers often state "0.2 mm, 2 walls, 2 % infill" up front.
3. If unstated, ask before defaulting.

The CLI doesn't yet have `--infill` / `--walls` flags. Until it does, override via a patched process profile (see `profiles.md` for the pattern).

**Real example.** A 3-tier vase-style "perfume stand" was profiled "0.2 mm layer, 2 walls, 2 % infill". Slicing a remix at the CLI's 15 % default produced 21 h / 757 g — about 7× the original. Re-sliced at 2 % gyroid / 2 walls dropped it to 6.5 h / 211 g, in line with the source. Lesson: trust the original designer's profile for shell-style decorative pieces; don't blanket-apply your defaults.

## Bambu Studio version-mapping dialog after opening a CLI 3MF

When the user opens an OrcaSlicer-produced 3MF in Bambu Studio, BS will show an info dialog mapping a few keys (`ensure_vertical_shell_thickness` → `ensure_all`, `rectilinear` ironing → `zig-zag`, etc.). This is harmless schema drift — print parameters are preserved. Tell the user to click OK and proceed.

## Detect multi-plate 3MFs with identical parts and offer vertical stacking

When the user hands a `.3mf` with multiple plates, **inspect the part files before slicing each plate separately**. If two or more plates are the same flat-topped functional part (drawers, lids, bins, plates), default to offering the Pattern A vertical stack from `SKILL.md` Section 4 instead of N separate prints.

```bash
unzip -l source.3mf | grep '3D/Objects/'
mkdir -p /tmp/3mf-inspect && cd /tmp/3mf-inspect && unzip -qo source.3mf
ls -la 3D/Objects/         # equal byte sizes are a strong identical-part signal
cksum  3D/Objects/*.model  # confirm geometric equality
```

Why it matters: a 4-plate organizer download where plates 2 and 3 are the same drawer collapses to a single 1- or 2-plate print job. That's N-1 fewer bed preps and N-1 fewer first-layer-failure risk windows. The user wants this — make sure to surface it before queueing the original separate plates.

Failure modes to flag before stacking:

- Lower part has any top-surface overhang or curvature → Pattern A will not bond cleanly. Fall back to Pattern B (sequential by-object) or separate plates.
- Upper part footprint exceeds lower top → overhang on the print, abort the stack offer.
- Combined stack height > printer Z range → split into smaller stacks.
- Different filaments per layer → requires AMS swap or pause-resume. For separable stacks this is expected: use an incompatible release interface and verify AMS materials first.

Tooling status: pre-composing a stacked STL from N inputs is a candidate CLI enhancement (`--stack-z` taking N inputs and writing a Z-offset assembly STL). Today the operation is performed in Bambu Studio's UI via Merge → Move on Z.

## Separable vertical stacks need a release interface

When the user asks to "stack" duplicate parts, do **not** assume same-material contact is acceptable. Same-material vertical stacks can fuse into one part. For separable PLA stacks, insert a one-layer PETG release/interface sheet at the contact plane and assign it to a PETG filament; for PETG parts, use PLA as the incompatible release material.

Failure mode: same-filament Pattern A stack bonds at the flat interface. Root cause: the upper first layer is printed directly onto the previous part's top surface. Fix: make the stack an assembly with bottom part = model filament, one release layer = incompatible filament, top part = model filament, then preview the interface layer before printing. Save the generated 3MF under the configured generated-artifact output directory, not Downloads. Why it matters: "stacked" usually means batched but separable, not fused.

## Two-up plates when geometry permits

After confirming a single copy fits the bed with margin, **proactively offer a 2-up (or N-up) pre-arranged 3MF** — most users like amortizing bed prep across copies. Pass the same STL multiple times to the CLI:

```bash
bun run "$BAMBU_SLICER_CLI_DIR/cli.ts" --input "model.stl model.stl" --output 2up.3mf
```

The slicer auto-arranges. Don't auto-bake the multi-up though — confirm copy count first, since some prints (large, brittle, or one-of-a-kind) shouldn't be doubled.

## Tested constants for tray/shelf/riser-style functional parts

For shelf risers, drawer organizers, and other "tray" pieces, these constants have been validated on real parts:

- **Wall fillet radius:** 2 mm. Use `hull()` of 8 corner spheres at `$fn=16` — much faster than `minkowski()`.
- **Lip fillet radius:** 0.9 mm with 2 mm lip thickness.
- **Front/side lip height:** 8 mm.
- **Rear lip height:** 12 mm (when the back wall is reduced to a lip rather than a full wall).
- **Rib spacing:** 4 mm centers, 0.6 mm groove depth, 1.6 mm groove width — and inset rib z-range by the fillet radius so ribs don't intrude into the rounded edges.

These are starting points, not laws. Adjust for the specific part's loading and aesthetics.

## Where this list lives — and why

This file is the durable home for printer + MakerWorld + slicer-CLI gotchas. It's part of the skill, not project memory, so improvements auto-load next session.

When you discover a new lesson, append it here and keep the entry tight: failure mode → root cause → fix → why it matters.
