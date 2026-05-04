# Profile Reference

Default profile names used by the bundled CLI, all targeting a Bambu Lab P2S with a 0.4 mm nozzle. Override via `BAMBU_MACHINE_PROFILE` and the maps in `scripts/cli/profiles.ts` for other Bambu printers. Generated output defaults to a temp generated-artifact directory; override with `BAMBU_OUTPUT_DIR` for a user-chosen generated-artifact directory.

## Quality profiles (`--quality`, 0.4 mm nozzle)

| Flag | Profile | Use case |
|------|---------|----------|
| 0.08 | 0.08 mm High Quality | Display pieces, fine detail |
| 0.12 | 0.12 mm High Quality | Good detail, reasonable speed |
| 0.16 | 0.16 mm Standard | Balanced |
| 0.20 | 0.20 mm Standard (default) | Everyday prints |
| 0.24 | 0.24 mm Standard | Fast, functional parts |

## Filament profiles (`--filament`)

| Name | Material | Notes |
|------|----------|-------|
| Bambu PLA Basic | PLA (default, unless `BAMBU_DEFAULT_FILAMENT` is set) | General purpose. Profile color is not AMS truth. |
| Bambu PLA Matte | PLA | Nice surface finish |
| Bambu PETG Basic | PETG | Heat/moisture resistant |
| Generic PLA | PLA | Third-party PLA |
| Generic PETG | PETG | Third-party PETG |

## AMS and profile color caveat

The CLI loads filament *profiles*; it does not query the printer AMS by itself. The orange-ish color seen in generated 3MFs comes from Bambu/Orca profile metadata, not from a live spool lookup. When `bambu-printer` MCP AMS tools are available, query AMS first and choose the profile that matches the loaded material. When they are unavailable, preserve the source 3MF material/profile or ask the user.

## Adding new profiles

The maps live in `scripts/cli/profiles.ts` (`PROCESS_MAP`, `FILAMENT_MAP`). To support a different Bambu printer (X1C, P1S, A1, …):

1. Find the printer's profile JSON files in `~/Library/Application Support/BambuStudio/system/BBL/{machine,process,filament}/`.
2. Update `MACHINE_PROFILE` and the maps to match. Suffix conventions vary (`@BBL P2S`, `@BBL X1C`, …).
3. Override at runtime via env vars without editing source: `BAMBU_MACHINE_PROFILE="Bambu Lab X1 Carbon 0.4 nozzle.json"` and similar.

## Process profile overrides for remixes

The CLI doesn't yet expose `--infill` / `--walls` flags. To override infill density, wall count, or shell layers when remixing, write a tmp lean-process JSON that overrides the relevant keys (`sparse_infill_density`, `wall_loops`, `sparse_infill_pattern`, `top_shell_layers`, `bottom_shell_layers`) and pass it as the second `--load-settings` semicolon entry. The pattern matches `patchMachineProfile` in `scripts/cli/slicer.ts`.

See `gotchas.md` for the failure mode that motivates this.
