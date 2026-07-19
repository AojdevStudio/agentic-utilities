import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { findBrokenPackedLinks } from "./lib/package-links.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync("bun", ["pm", "pack", "--dry-run", "--ignore-scripts"], {
  cwd: root,
  encoding: "utf8",
});

if (result.status !== 0) {
  process.stderr.write(result.stderr || result.stdout);
  process.exit(result.status ?? 1);
}

const packedPaths = result.stdout
  .split("\n")
  .map((line) => line.match(/^packed\s+\S+\s+(.+)$/)?.[1])
  .filter(Boolean);
const packedPathSet = new Set(packedPaths);
const requiredPaths = [
  "skills/diataxis-docs-site/SKILL.md",
  "skills/diataxis-docs-site/agents/openai.yaml",
  "skills/diataxis-docs-site/assets/aoj-starlight/gitignore.template",
  "skills/diataxis-docs-site/scripts/create_site.py",
  "skills/github-wiki/SKILL.md",
  "skills/github-wiki/agents/openai.yaml",
  "skills/herdr-fleet/SKILL.md",
  "skills/herdr-fleet/README.md",
  "skills/herdr-fleet/launch-fleet.md",
  "skills/herdr-fleet/protocols.md",
  "skills/herdr-fleet/fixtures/compaction-failures.json",
  "skills/herdr-fleet/fixtures/fleet-state-race.json",
  "skills/herdr-fleet/fixtures/project-key-collision.json",
  "skills/herdr-fleet/fixtures/watcher-identity.json",
  "skills/herdr-fleet/scripts/consume-events.mjs",
  "skills/herdr-fleet/scripts/fleet-labels.mjs",
  "skills/herdr-fleet/scripts/fleet-state.mjs",
  "skills/herdr-fleet/scripts/fleet-state.test.mjs",
  "skills/herdr-fleet/scripts/resolve-project-key.mjs",
  "skills/herdr-fleet/scripts/watch-fleet.mjs",
  "scripts/lib/package-links.mjs",
  "scripts/lib/package-links.test.mjs",
];
const missing = requiredPaths.filter((requiredPath) => !packedPathSet.has(requiredPath));
const forbidden = packedPaths.filter(
  (packedPath) => /(?:^|\/)__pycache__(?:\/|$)/.test(packedPath) || /\.py[cod]$/.test(packedPath),
);

function markdownFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return markdownFiles(entryPath);
    return entry.isFile() && entry.name.endsWith(".md") ? [entryPath] : [];
  });
}

const brokenLinks = [];
for (const markdownPath of markdownFiles(path.join(root, "skills/herdr-fleet"))) {
  const sourcePath = path.relative(root, markdownPath).split(path.sep).join(path.posix.sep);
  if (!packedPathSet.has(sourcePath)) {
    brokenLinks.push(`${sourcePath} is not packed`);
    continue;
  }
  brokenLinks.push(
    ...findBrokenPackedLinks({
      sourcePath,
      content: readFileSync(markdownPath, "utf8"),
      packedPathSet,
      readTarget: (targetPath) => readFileSync(path.join(root, targetPath), "utf8"),
    }),
  );
}

if (missing.length > 0 || forbidden.length > 0 || brokenLinks.length > 0) {
  if (missing.length > 0) process.stderr.write(`Missing required package files:\n${missing.join("\n")}\n`);
  if (forbidden.length > 0)
    process.stderr.write(`Forbidden generated artifacts in package:\n${forbidden.join("\n")}\n`);
  if (brokenLinks.length > 0) process.stderr.write(`Broken herdr-fleet links:\n${brokenLinks.join("\n")}\n`);
  process.exit(1);
}

process.stdout.write("✓ package contents and herdr-fleet links are valid\n");
