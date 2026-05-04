#!/usr/bin/env bun

import { existsSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { listProfiles, resolveProfiles } from "./profiles.js";
import { runSlicer } from "./slicer.js";

const DEFAULT_OUTPUT_DIR = process.env.BAMBU_OUTPUT_DIR || join(tmpdir(), "bambu-slicer-generated");
const DEFAULT_FILAMENT = process.env.BAMBU_DEFAULT_FILAMENT || "Bambu PLA Basic";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    input: { type: "string" },
    output: { type: "string" },
    quality: { type: "string", default: "0.20" },
    filament: { type: "string", default: DEFAULT_FILAMENT },
    "list-profiles": { type: "boolean", default: false },
  },
  strict: true,
});

function timestamp(): string {
  return new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}Z$/, "Z");
}

function defaultOutputFor(inputFiles: string[]): string {
  mkdirSync(DEFAULT_OUTPUT_DIR, { recursive: true });
  const stem =
    inputFiles.length === 1
      ? basename(inputFiles[0], extname(inputFiles[0])).replace(/[^A-Za-z0-9._-]+/g, "_")
      : `combined-${inputFiles.length}-objects`;
  const candidate = join(DEFAULT_OUTPUT_DIR, `${stem}.3mf`);
  if (!existsSync(candidate)) return candidate;
  return join(DEFAULT_OUTPUT_DIR, `${stem}-${timestamp()}.3mf`);
}

if (values["list-profiles"]) {
  const { qualities, filaments } = listProfiles();
  console.log("Quality profiles (--quality):");
  for (const q of qualities) console.log(`  ${q}`);
  console.log("\nFilament profiles (--filament):");
  for (const f of filaments) console.log(`  ${f}`);
  process.exit(0);
}

if (!values.input) {
  console.error('Usage: bun run cli.ts --input <stl> [--output <3mf>] [--quality 0.20] [--filament "Bambu PLA Basic"]');
  console.error("       bun run cli.ts --list-profiles");
  console.error(`       default output dir: ${DEFAULT_OUTPUT_DIR}`);
  process.exit(1);
}

// Resolve every path to an absolute one so OrcaSlicer's argv parser can't
// mistake a filename like "--export-3mf=evil.3mf" for a flag.
const inputFiles = values.input
  .split(/\s+/)
  .filter(Boolean)
  .map((f) => resolve(f));
const outputFile = resolve(values.output || defaultOutputFor(inputFiles));
const profiles = resolveProfiles({
  quality: values.quality!,
  filament: values.filament!,
});

console.log(`Slicing ${inputFiles.length} file(s)...`);
console.log(`  Quality: ${values.quality}`);
console.log(`  Filament: ${values.filament}`);
console.log(`  Output: ${outputFile}`);

const result = await runSlicer({
  inputFiles,
  outputFile,
  ...profiles,
});

if (result.success) {
  const stat = Bun.file(result.outputFile);
  const sizeKB = Math.round((await stat.size) / 1024);
  console.log(`\nSuccess! Output: ${result.outputFile} (${sizeKB} KB)`);
} else {
  console.error(`\nSlicing failed: ${result.error}`);
  process.exit(1);
}
