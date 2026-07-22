#!/usr/bin/env node
import { readFile } from "node:fs/promises";

const DEPENDENCY_GROUPS = ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"];

export function dependencyMetadataMismatches(manifest, lockfile) {
  const root = lockfile?.packages?.[""] ?? {};
  return DEPENDENCY_GROUPS.flatMap((group) => {
    const expected = manifest[group] ?? {};
    const actual = root[group] ?? {};
    return [...new Set([...Object.keys(expected), ...Object.keys(actual)])]
      .sort()
      .filter((name) => expected[name] !== actual[name])
      .map(
        (name) =>
          `${group}.${name}: package.json=${JSON.stringify(expected[name])}, package-lock.json=${JSON.stringify(actual[name])}`,
      );
  });
}

async function main() {
  const [manifest, lockfile] = await Promise.all([
    readFile("package.json", "utf8").then(JSON.parse),
    readFile("package-lock.json", "utf8").then(JSON.parse),
  ]);
  const mismatches = dependencyMetadataMismatches(manifest, lockfile);
  if (mismatches.length > 0) {
    process.stderr.write(`Package lock metadata differs from package.json:\n- ${mismatches.join("\n- ")}\n`);
    process.exit(1);
  }
  process.stdout.write("✓ package-lock root dependency metadata matches package.json\n");
}

if (import.meta.path === Bun.main) await main();
