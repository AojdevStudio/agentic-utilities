#!/usr/bin/env node
import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const [, , name, ...descriptionParts] = process.argv;
const kebabCase = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

if (!name || !kebabCase.test(name)) {
  console.error("Usage: npm run new:extension -- <kebab-name> [description]");
  console.error('Example: npm run new:extension -- dirty-repo-guard "Warn before risky git state changes"');
  process.exit(1);
}

const description = descriptionParts.join(" ") || `Pi extension: ${name}`;
const dir = join("extensions", name);
const toolName = name.replaceAll("-", "_");

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
  join(dir, "index.ts"),
  `import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";\n` +
    `import { Type } from "typebox";\n\n` +
    `const EXTENSION_NAME = ${JSON.stringify(name)};\n\n` +
    `export default function (pi: ExtensionAPI) {\n` +
    `  pi.registerCommand(${JSON.stringify(name)}, {\n` +
    `    description: ${JSON.stringify(description)},\n` +
    `    handler: async (_args, ctx) => {\n` +
    `      if (ctx.hasUI) ctx.ui.notify(EXTENSION_NAME + " loaded", "info");\n` +
    `    },\n` +
    `  });\n\n` +
    `  pi.registerTool({\n` +
    `    name: ${JSON.stringify(toolName)},\n` +
    `    label: ${JSON.stringify(name)},\n` +
    `    description: ${JSON.stringify(description)},\n` +
    `    parameters: Type.Object({\n` +
    `      message: Type.Optional(Type.String({ description: "Optional message." })),\n` +
    `    }),\n` +
    `    async execute(_toolCallId, params) {\n` +
    `      const text = params.message ? EXTENSION_NAME + ": " + params.message : EXTENSION_NAME + " ready.";\n` +
    `      return {\n` +
    `        content: [{ type: "text", text }],\n` +
    `        details: { ok: true },\n` +
    `      };\n` +
    `    },\n` +
    `  });\n` +
    `}\n`,
  "utf8",
);

await writeFile(
  join(dir, "README.md"),
  `# ${name}\n\n${description}\n\n## Provides\n\n- Command: \`/${name}\`\n- Tool: \`${toolName}\`\n\n## Notes\n\nDocument events, dependencies, state, and safety constraints here.\n`,
  "utf8",
);

console.log(`Created ${dir}`);
console.log("Next: update docs/catalog.md, then run npm run check.");
