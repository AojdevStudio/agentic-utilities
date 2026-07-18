import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: root, encoding: "utf8" });

if (result.status !== 0) {
  process.stderr.write(result.stderr || result.stdout);
  process.exit(result.status ?? 1);
}

const [pack] = JSON.parse(result.stdout);
const packedPaths = pack.files.map((file) => file.path);
const packedPathSet = new Set(packedPaths);
const requiredPaths = [
  "skills/diataxis-docs-site/SKILL.md",
  "skills/diataxis-docs-site/agents/openai.yaml",
  "skills/diataxis-docs-site/assets/aoj-starlight/gitignore.template",
  "skills/diataxis-docs-site/scripts/create_site.py",
  "skills/github-wiki/SKILL.md",
  "skills/github-wiki/agents/openai.yaml",
];
const missing = requiredPaths.filter((requiredPath) => !packedPathSet.has(requiredPath));
const forbidden = packedPaths.filter(
  (packedPath) => /(?:^|\/)__pycache__(?:\/|$)/.test(packedPath) || /\.py[cod]$/.test(packedPath),
);

if (missing.length > 0 || forbidden.length > 0) {
  if (missing.length > 0) console.error(`Missing required package files:\n${missing.join("\n")}`);
  if (forbidden.length > 0) console.error(`Forbidden generated artifacts in package:\n${forbidden.join("\n")}`);
  process.exit(1);
}

console.log("✓ required skill package files are included");
