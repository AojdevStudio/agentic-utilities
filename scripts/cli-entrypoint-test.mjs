import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Regression for a URL-unsafe direct-execution guard: a naive
// `import.meta.url === \`file://${process.argv[1]}\`` comparison fails
// silently for any checkout path containing a space (import.meta.url
// percent-encodes it, the raw argv string does not), so `node
// validate-plugin-ports.mjs` would exit 0 having never called main().
//
// This copies the real script and its lib dependencies into a temp
// directory whose path contains a space, symlinks node_modules alongside
// it so "yaml" still resolves, and runs it as a real subprocess. There is
// no claude-code/plugins directory in the copy, so a correctly-invoked
// main() must fail loudly with "plugins directory not found" — proof that
// main() actually ran, not just that the subprocess exited.

const scriptsDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)));
const repoRoot = path.resolve(scriptsDir, "..");

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cli-entrypoint-"));
const spacedDir = path.join(tmp, "path with a space");
fs.mkdirSync(path.join(spacedDir, "scripts", "lib"), { recursive: true });

for (const file of ["validate-plugin-ports.mjs"]) {
  fs.copyFileSync(path.join(scriptsDir, file), path.join(spacedDir, "scripts", file));
}
for (const file of ["bundle-refs.mjs", "frontmatter.mjs"]) {
  fs.copyFileSync(path.join(scriptsDir, "lib", file), path.join(spacedDir, "scripts", "lib", file));
}
fs.symlinkSync(path.join(repoRoot, "node_modules"), path.join(spacedDir, "node_modules"));

const targetScript = path.join(spacedDir, "scripts", "validate-plugin-ports.mjs");
const result = spawnSync(process.execPath, [targetScript], { encoding: "utf8" });

fs.rmSync(tmp, { recursive: true, force: true });

assert.notEqual(result.status, 0, "main() must run and fail against the fixture's absent claude-code/plugins dir");
assert.match(
  result.stderr,
  /plugins directory not found/,
  "stderr must show main() actually executed listPlugins(), proving the space-containing path was recognized as direct execution",
);

console.log("CLI entrypoint direct-execution fixtures passed");
