#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pluginsRoot = path.join(repoRoot, "codex", "plugins");
const catalogPath = path.join(repoRoot, "docs", "catalog.md");
const SEMVER_RE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const KEBAB_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const JUNK_NAMES = new Set([".DS_Store", "Thumbs.db", "node_modules", "__pycache__", ".env"]);
const failures = [];

function fail(plugin, message) {
  failures.push({ plugin, message });
}

function isInside(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..");
}

function readJson(file, plugin) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    fail(plugin, `${path.relative(repoRoot, file)} is not valid JSON: ${error.message}`);
    return null;
  }
}

function walk(dir, visitor) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    visitor(full, entry);
    if (entry.isDirectory() && entry.name !== ".git" && !JUNK_NAMES.has(entry.name)) walk(full, visitor);
  }
}

function frontmatter(file, plugin) {
  const text = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  const match = /^---\n([\s\S]*?)\n---(?:\n|$)/.exec(text);
  if (!match) {
    fail(plugin, `${path.relative(repoRoot, file)} is missing YAML frontmatter`);
    return null;
  }
  const fields = new Map();
  for (const line of match[1].split("\n")) {
    const field = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(line);
    if (field) fields.set(field[1], field[2].trim().replace(/^["']|["']$/g, ""));
  }
  return fields;
}

function referenceExists(pluginRoot, sourceFile, reference) {
  const clean = reference.split("#", 1)[0].split("?", 1)[0].replace(/^\.\//, "");
  if (!clean || /^(?:https?:|mailto:|#)/.test(clean)) return true;
  const wildcard = clean.search(/[?*]/);
  const withoutGlob = wildcard === -1 ? clean : clean.slice(0, wildcard);
  const targetFragment =
    wildcard === -1 ? withoutGlob : withoutGlob.endsWith("/") ? withoutGlob.slice(0, -1) : path.dirname(withoutGlob);
  const bases = [path.dirname(sourceFile), pluginRoot];
  if (/^(?:scripts|assets|skills)\//.test(clean)) bases.reverse();
  return bases.some((base) => {
    const resolved = path.resolve(base, targetFragment || ".");
    return isInside(pluginRoot, resolved) && fs.existsSync(resolved);
  });
}

function checkReferences(plugin, pluginRoot, file) {
  const text = fs.readFileSync(file, "utf8");
  const references = new Set();
  for (const match of text.matchAll(/\]\(([^)\s]+)\)/g)) references.add(match[1]);
  for (const match of text.matchAll(/(?:^|[\s`"'$/])((?:scripts|references|assets|skills)\/[A-Za-z0-9._*?/-]+)/gm)) {
    references.add(match[1]);
  }
  for (const reference of references) {
    if (!referenceExists(pluginRoot, file, reference)) {
      fail(plugin, `${path.relative(repoRoot, file)} references missing local path: ${reference}`);
    }
  }
}

if (!fs.existsSync(pluginsRoot)) {
  console.log("Validated 0 Codex plugins (codex/plugins is absent)");
  process.exit(0);
}

const catalog = fs.readFileSync(catalogPath, "utf8");
const plugins = fs
  .readdirSync(pluginsRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();

for (const plugin of plugins) {
  const pluginRoot = path.join(pluginsRoot, plugin);
  const manifestPath = path.join(pluginRoot, ".codex-plugin", "plugin.json");
  const readmePath = path.join(pluginRoot, "README.md");

  if (!KEBAB_RE.test(plugin)) fail(plugin, "plugin directory must be kebab-case");
  if (!fs.existsSync(manifestPath)) {
    fail(plugin, "missing .codex-plugin/plugin.json");
    continue;
  }
  const manifest = readJson(manifestPath, plugin);
  if (!manifest) continue;
  if (manifest.name !== plugin) fail(plugin, `manifest name '${manifest.name}' must match directory '${plugin}'`);
  if (typeof manifest.version !== "string" || !SEMVER_RE.test(manifest.version)) {
    fail(plugin, `manifest version '${manifest.version}' is not valid semver`);
  }
  for (const field of ["description", "license"]) {
    if (typeof manifest[field] !== "string" || manifest[field].trim() === "")
      fail(plugin, `manifest ${field} is required`);
  }
  if (!fs.existsSync(readmePath)) fail(plugin, "missing README.md");

  if (typeof manifest.skills !== "string" || manifest.skills.trim() === "") {
    fail(plugin, "manifest skills must declare a local directory");
  } else {
    const skillsDir = path.resolve(pluginRoot, manifest.skills);
    if (!isInside(pluginRoot, skillsDir) || !fs.existsSync(skillsDir) || !fs.statSync(skillsDir).isDirectory()) {
      fail(plugin, `manifest skills directory is missing or escapes the plugin: ${manifest.skills}`);
    } else {
      const skillFiles = [];
      walk(skillsDir, (full, entry) => {
        if (entry.isFile() && entry.name === "SKILL.md") skillFiles.push(full);
      });
      if (skillFiles.length === 0) fail(plugin, "declared skills directory contains no SKILL.md files");
      for (const skillFile of skillFiles) {
        const fields = frontmatter(skillFile, plugin);
        if (fields) {
          const expected = path.basename(path.dirname(skillFile));
          if (fields.get("name") !== expected) {
            fail(plugin, `${path.relative(repoRoot, skillFile)} name must match parent directory '${expected}'`);
          }
          if (!fields.get("description")) fail(plugin, `${path.relative(repoRoot, skillFile)} requires description`);
        }
        checkReferences(plugin, pluginRoot, skillFile);
      }
    }
  }

  if (fs.existsSync(readmePath)) checkReferences(plugin, pluginRoot, readmePath);
  const catalogPattern = new RegExp(
    `^\\|\\s*\\\`${plugin.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\\`\\s*\\|\\s*Codex Plugin\\s*\\|\\s*\\\`codex/plugins/${plugin}/\\.codex-plugin/plugin\\.json\\\`\\s*\\|`,
    "m",
  );
  if (!catalogPattern.test(catalog)) fail(plugin, "missing exact Codex Plugin registration in docs/catalog.md");

  walk(pluginRoot, (full, entry) => {
    if (JUNK_NAMES.has(entry.name) || (entry.isFile() && /\.(?:pyc|db|db-wal|db-shm)$/.test(entry.name))) {
      fail(plugin, `junk artifact present: ${path.relative(pluginRoot, full)}`);
    }
  });

  const fixtureVerifier = path.join(pluginRoot, "scripts", "verify_fixtures.py");
  if (fs.existsSync(fixtureVerifier)) {
    const result = spawnSync("python3", [fixtureVerifier], {
      cwd: repoRoot,
      encoding: "utf8",
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
    });
    if (result.status !== 0) {
      fail(plugin, `fixture verifier failed: ${(result.stderr || result.stdout).trim()}`);
    } else if (result.stdout.trim()) {
      console.log(`[${plugin}] ${result.stdout.trim()}`);
    }
  }
}

if (failures.length > 0) {
  console.error(`Codex plugin validation failed (${failures.length} issue(s)):`);
  for (const { plugin, message } of failures) console.error(`- [${plugin}] ${message}`);
  process.exit(1);
}

console.log(`Validated ${plugins.length} Codex plugin${plugins.length === 1 ? "" : "s"}: ${plugins.join(", ")}`);
