#!/usr/bin/env node
import { readdir, readFile } from "node:fs/promises";
import { basename, join, relative } from "node:path";

const SKILL_NAME_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const DESCRIPTION_MAX_LENGTH = 1024;
const SKILL_ROOTS = ["skills", "claude-code/plugins"];

async function walk(dir) {
  const out = [];
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return out;
    throw error;
  }

  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await walk(path)));
    else if (entry.isFile() && entry.name === "SKILL.md") out.push(path);
  }
  return out;
}

function parseFrontmatter(content, filePath) {
  const normalized = content.replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---\n")) {
    throw new Error(`${filePath}: missing YAML frontmatter block`);
  }

  const end = normalized.indexOf("\n---", 4);
  if (end === -1) {
    throw new Error(`${filePath}: unterminated YAML frontmatter block`);
  }

  const lines = normalized.slice(4, end).split("\n");
  const fields = new Map();

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    if (line.startsWith(" ") || line.startsWith("\t")) continue;

    const match = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!match) continue;

    const [, key, rawValue] = match;
    const value = rawValue.trim();
    if (["|", ">", "|-", ">-"].includes(value)) {
      const block = [];
      for (let j = i + 1; j < lines.length; j += 1) {
        const next = lines[j];
        if (next && !next.startsWith(" ") && !next.startsWith("\t")) break;
        block.push(next.trim());
        i = j;
      }
      fields.set(key, block.filter(Boolean).join(" ").trim());
    } else {
      fields.set(key, value.replace(/^['\"]|['\"]$/g, ""));
    }
  }

  return fields;
}

async function validateSkillsShConfig(skillNames) {
  const file = "skills.sh.json";
  let parsed;
  try {
    parsed = JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    return [`${file}: ${error.message}`];
  }

  const errors = [];
  if (!Array.isArray(parsed.groupings) || parsed.groupings.length === 0) {
    errors.push(`${file}: groupings must be a non-empty array`);
    return errors;
  }

  for (const [index, group] of parsed.groupings.entries()) {
    if (!group || typeof group !== "object") {
      errors.push(`${file}: groupings[${index}] must be an object`);
      continue;
    }
    if (typeof group.title !== "string" || group.title.trim() === "") {
      errors.push(`${file}: groupings[${index}].title must be a non-empty string`);
    }
    if (!Array.isArray(group.skills) || group.skills.length === 0) {
      errors.push(`${file}: groupings[${index}].skills must be a non-empty array`);
      continue;
    }
    for (const skill of group.skills) {
      if (typeof skill !== "string" || skill.trim() === "") {
        errors.push(`${file}: groupings[${index}].skills contains a non-string or empty entry`);
      } else if (!skillNames.has(skill.toLowerCase())) {
        errors.push(`${file}: grouped skill '${skill}' was not found in the skills CLI inventory`);
      }
    }
  }

  return errors;
}

const files = (await Promise.all(SKILL_ROOTS.map((root) => walk(root)))).flat().sort();
const errors = [];
const warnings = [];
const byName = new Map();

for (const file of files) {
  let fields;
  try {
    fields = parseFrontmatter(await readFile(file, "utf8"), file);
  } catch (error) {
    errors.push(error.message);
    continue;
  }

  const name = fields.get("name");
  const description = fields.get("description");
  const expectedName = basename(file.replace(/\/SKILL\.md$/, ""));

  if (!name) errors.push(`${file}: missing required frontmatter field: name`);
  else {
    if (name !== expectedName) errors.push(`${file}: name '${name}' must match parent directory '${expectedName}'`);
    if (!SKILL_NAME_RE.test(name)) errors.push(`${file}: name '${name}' must be lowercase kebab-case`);
    const existing = byName.get(name) ?? [];
    existing.push(file);
    byName.set(name, existing);
  }

  if (!description) errors.push(`${file}: missing required frontmatter field: description`);
  else if (description.length > DESCRIPTION_MAX_LENGTH) {
    errors.push(`${file}: description is ${description.length} chars; max is ${DESCRIPTION_MAX_LENGTH}`);
  }
}

for (const [name, paths] of byName.entries()) {
  if (paths.length > 1) {
    warnings.push(`duplicate skill name '${name}' appears in multiple distribution lanes: ${paths.join(", ")}`);
  }
}

errors.push(...(await validateSkillsShConfig(new Set(byName.keys()))));

if (errors.length > 0) {
  console.error("Agent Skills validation failed:\n");
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

if (warnings.length > 0) {
  console.warn("Agent Skills validation warnings:");
  for (const warning of warnings) console.warn(`- ${warning}`);
  console.warn("");
}

console.log(
  `Validated ${files.length} SKILL.md file${files.length === 1 ? "" : "s"} (${byName.size} unique skill names):`,
);
for (const file of files) console.log(`- ${relative(process.cwd(), file)}`);
