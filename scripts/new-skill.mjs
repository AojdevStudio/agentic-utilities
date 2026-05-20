#!/usr/bin/env node
import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [, , name, ...descriptionParts] = process.argv;
const kebabCase = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

if (!name || !kebabCase.test(name)) {
  console.error("Usage: npm run new:skill -- <kebab-name> [description]");
  console.error('Example: npm run new:skill -- release-checklist "Use when preparing a release."');
  process.exit(1);
}

const description = descriptionParts.join(" ") || `Use when working on ${name}.`;
if (description.length > 1024) {
  console.error("Skill description must be 1024 characters or less.");
  process.exit(1);
}

const dir = join("skills", name);

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

if (await pathExists(dir)) {
  console.error(`Refusing to overwrite existing directory: ${dir}`);
  process.exit(1);
}

await mkdir(dir, { recursive: true });

await writeFile(
  join(dir, "SKILL.md"),
  `---\nname: ${name}\ndescription: ${description}\n---\n\n# ${name}\n\n## When to use\n\n${description}\n\n## Process\n\n1. Inspect the relevant files.\n2. Make the smallest safe change.\n3. Verify the result.\n\n## References\n\nAdd links to files under \`references/\` when this skill grows.\n`,
  "utf8",
);

await writeFile(join(dir, "README.md"), `# ${name}\n\n${description}\n`, "utf8");

console.log(`Created ${dir}`);
console.log("Next: update docs/catalog.md, then run npm run list.");
