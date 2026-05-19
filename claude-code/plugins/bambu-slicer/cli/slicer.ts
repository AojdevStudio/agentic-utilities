import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Keys that must never be copied from one object to another via dynamic
// assignment — assigning to them invokes setters that mutate the prototype.
const PROTO_KEYS = new Set(["__proto__", "constructor", "prototype"]);

// OrcaSlicer is used instead of Bambu Studio CLI because Bambu Studio CLI has a
// known segfault bug with P2S 0.4 mm nozzle profiles (OrcaSlicer issue #9636).
// Override via ORCA_CLI_PATH for non-default install locations.
const ORCA_CLI = process.env.ORCA_CLI_PATH || "/Applications/OrcaSlicer.app/Contents/MacOS/OrcaSlicer";

// Simplified gcode templates compatible with OrcaSlicer's parser.
// Bambu Studio profiles use variables like `min_vitrification_temperature` that
// OrcaSlicer doesn't support. These simplified versions produce valid gcode —
// the actual start/end gcode lives in the printer firmware.
const ORCA_MACHINE_START_GCODE =
  "G28\\nG1 Z5 F5000\\nM104 S[nozzle_temperature_initial_layer]\\nM140 S[bed_temperature_initial_layer_single]\\nM109 S[nozzle_temperature_initial_layer]\\nM190 S[bed_temperature_initial_layer_single]\\nG92 E0\\n";
const ORCA_MACHINE_END_GCODE = "G1 E-2 F2400\\nG28 X\\nM104 S0\\nM140 S0\\nM84\\n";
const ORCA_LAYER_CHANGE_GCODE = "G92 E0\\n;LAYER_CHANGE\\n;Z:[layer_z]\\nM73 L[layer_num]\\n";

export interface SliceInput {
  inputFiles: string[];
  outputFile: string;
  machine: string;
  process: string;
  filament: string;
}

export interface SliceResult {
  success: boolean;
  outputFile: string;
  error?: string;
}

export function buildSliceArgs(input: SliceInput): string[] {
  return [
    ...input.inputFiles,
    "--load-settings",
    `${input.machine};${input.process}`,
    "--load-filaments",
    input.filament,
    "--arrange",
    "1",
    "--orient",
    "1",
    "--slice",
    "0",
    "--export-3mf",
    input.outputFile,
  ];
}

/**
 * Patches a Bambu Studio machine profile to be OrcaSlicer-compatible.
 * Merges template includes inline and replaces Bambu Studio-specific gcode
 * with simplified versions that OrcaSlicer can parse.
 */
function patchMachineProfile(machineProfilePath: string): string {
  const profile = JSON.parse(readFileSync(machineProfilePath, "utf-8"));
  const profileDir = machineProfilePath.replace(/\/[^/]+$/, "");

  // Resolve `inherits` chain — OrcaSlicer can't follow inherits across a
  // patched profile written outside the vendor tree, so merge parent fields
  // inline. Child fields win over parent.
  let cursor = profile.inherits as string | undefined;
  const seen = new Set<string>();
  while (cursor && !seen.has(cursor)) {
    seen.add(cursor);
    const parentPath = join(profileDir, `${cursor}.json`);
    if (!existsSync(parentPath)) break;
    const parent = JSON.parse(readFileSync(parentPath, "utf-8"));
    for (const [k, v] of Object.entries(parent)) {
      if (PROTO_KEYS.has(k)) continue;
      if (!Object.hasOwn(profile, k) && !["name", "instantiation", "type", "from", "inherits"].includes(k)) {
        profile[k] = v;
      }
    }
    cursor = parent.inherits;
  }
  delete profile.inherits;

  // Inline template includes
  if (profile.include) {
    for (const inc of profile.include) {
      const tpath = join(profileDir, `${inc}.json`);
      if (existsSync(tpath)) {
        const template = JSON.parse(readFileSync(tpath, "utf-8"));
        for (const [k, v] of Object.entries(template)) {
          if (PROTO_KEYS.has(k)) continue;
          if (!["name", "instantiation", "type", "from"].includes(k)) {
            profile[k] = v;
          }
        }
      }
    }
    delete profile.include;
  }

  // Replace Bambu Studio-specific gcode with OrcaSlicer-compatible versions
  profile.machine_start_gcode = ORCA_MACHINE_START_GCODE;
  profile.machine_end_gcode = ORCA_MACHINE_END_GCODE;
  profile.layer_change_gcode = ORCA_LAYER_CHANGE_GCODE;

  // Ensure nozzle_volume_type exists
  if (!profile.nozzle_volume_type) {
    profile.nozzle_volume_type = ["0"];
  }

  const tmpDir = mkdtempSync(join(tmpdir(), "bambu-slicer-"));
  const patchedPath = join(tmpDir, "machine-patched.json");
  writeFileSync(patchedPath, JSON.stringify(profile, null, 2));
  return patchedPath;
}

export async function runSlicer(input: SliceInput): Promise<SliceResult> {
  if (!existsSync(ORCA_CLI)) {
    return {
      success: false,
      outputFile: input.outputFile,
      error: `OrcaSlicer CLI not found at ${ORCA_CLI}. Install: brew install --cask orcaslicer (or set ORCA_CLI_PATH)`,
    };
  }

  for (const f of input.inputFiles) {
    if (!existsSync(f)) {
      return {
        success: false,
        outputFile: input.outputFile,
        error: `Input file not found: ${f}`,
      };
    }
  }

  // Patch machine profile for OrcaSlicer compatibility
  const patchedMachine = patchMachineProfile(input.machine);
  const patchedInput = { ...input, machine: patchedMachine };

  const args = buildSliceArgs(patchedInput);
  const proc = Bun.spawn([ORCA_CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });

  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;

  if (exitCode !== 0) {
    return {
      success: false,
      outputFile: input.outputFile,
      error: `OrcaSlicer exited with code ${exitCode}\nstdout: ${stdout}\nstderr: ${stderr}`,
    };
  }

  if (!existsSync(input.outputFile)) {
    return {
      success: false,
      outputFile: input.outputFile,
      error: `Slicing completed but output file not found at ${input.outputFile}\nstdout: ${stdout}\nstderr: ${stderr}`,
    };
  }

  return {
    success: true,
    outputFile: input.outputFile,
  };
}
