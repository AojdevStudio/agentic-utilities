#!/usr/bin/env node
import { readdir, stat } from "node:fs/promises";
import { basename, join, relative } from "node:path";

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function walk(dir) {
  if (!(await exists(dir))) return [];
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await walk(path)));
    else out.push(path);
  }
  return out;
}

async function extensionResources() {
  if (!(await exists("extensions"))) return [];
  const out = [];
  for (const entry of await readdir("extensions", { withFileTypes: true })) {
    const path = join("extensions", entry.name);
    if (entry.isFile() && entry.name.endsWith(".ts")) out.push(path);
    if (entry.isDirectory() && (await exists(join(path, "index.ts")))) out.push(join(path, "index.ts"));
  }
  return out;
}

const resources = [
  ...(await extensionResources()).map((path) => ["extension", path]),
  ...(await walk("skills")).filter((path) => basename(path) === "SKILL.md").map((path) => ["skill", path]),
  ...(await walk("prompts")).filter((path) => path.endsWith(".prompt.md")).map((path) => ["prompt", path]),
  ...(await walk("themes")).filter((path) => path.endsWith(".json")).map((path) => ["theme", path]),
];

if (resources.length === 0) {
  console.log("No Pi resources found.");
  process.exit(0);
}

const width = Math.max(...resources.map(([type]) => type.length), "type".length);
console.log(`${"type".padEnd(width)}  path`);
console.log(`${"-".repeat(width)}  ${"-".repeat(40)}`);
for (const [type, path] of resources) {
  console.log(`${type.padEnd(width)}  ${relative(process.cwd(), path)}`);
}
