#!/usr/bin/env node
/**
 * validate-plugin-ports.mjs
 *
 * Consistency guard for Claude Code plugin ports under `claude-code/plugins/`.
 * Every directory under that path is treated as a shipped plugin and must be
 * internally consistent: valid manifest, README, usable SKILL.md with
 * resolvable references, and wired into both marketplace.json and catalog.md.
 *
 * Exits non-zero with a per-plugin failure report on any inconsistency.
 * Wired into `npm run check` so plugin-port drift fails CI.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pluginsDir = path.join(repoRoot, "claude-code", "plugins");

/** Collected failures: { plugin, message }. */
const failures = [];
function fail(plugin, message) {
  failures.push({ plugin, message });
}

/** Directory names under claude-code/plugins/ — every one is a shipped plugin. */
function listPlugins() {
  if (!fs.existsSync(pluginsDir)) {
    throw new Error(`plugins directory not found: ${pluginsDir}`);
  }
  return fs
    .readdirSync(pluginsDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
}

// --- Check 1: every plugin has a valid .claude-plugin/plugin.json ---------
function checkPluginManifest(plugin) {
  const manifestPath = path.join(pluginsDir, plugin, ".claude-plugin", "plugin.json");
  if (!fs.existsSync(manifestPath)) {
    fail(plugin, "missing .claude-plugin/plugin.json");
    return null;
  }
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  } catch (err) {
    fail(plugin, `plugin.json does not parse as JSON: ${err.message}`);
    return null;
  }
  if (manifest.name !== plugin) {
    fail(plugin, `plugin.json name "${manifest.name}" does not match directory "${plugin}"`);
  }
  if (typeof manifest.description !== "string" || manifest.description.trim() === "") {
    fail(plugin, "plugin.json has no description");
    return null;
  }
  return manifest.description;
}

// --- Check 2: every plugin has a README.md -------------------------------
function checkReadme(plugin) {
  const readmePath = path.join(pluginsDir, plugin, "README.md");
  if (!fs.existsSync(readmePath)) {
    fail(plugin, "missing README.md");
  }
}

/**
 * Parse a leading `--- ... ---` YAML frontmatter block.
 * Returns the raw frontmatter text, or null when absent.
 */
function readFrontmatter(text) {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/.exec(text);
  return match ? match[1] : null;
}

function readFrontmatterField(frontmatter, field) {
  const lines = frontmatter.split(/\r?\n/);
  const index = lines.findIndex((line) => line.startsWith(`${field}:`));
  if (index === -1) return null;

  const raw = lines[index].slice(field.length + 1).trim();
  if (raw !== ">" && raw !== "|") return raw.replace(/^["']|["']$/g, "");

  const block = [];
  for (const line of lines.slice(index + 1)) {
    if (!/^\s+/.test(line)) break;
    block.push(line.trim());
  }
  return block.join(raw === ">" ? " " : "\n").trim();
}

/** Path to a plugin's canonical SKILL.md (skills/<plugin>/SKILL.md). */
function skillPath(plugin) {
  return path.join(pluginsDir, plugin, "skills", plugin, "SKILL.md");
}

// --- Check 3: SKILL.md has frontmatter with name matching the plugin -----
function checkSkillFrontmatter(plugin) {
  const file = skillPath(plugin);
  if (!fs.existsSync(file)) {
    fail(plugin, `missing SKILL.md at skills/${plugin}/SKILL.md`);
    return;
  }
  const frontmatter = readFrontmatter(fs.readFileSync(file, "utf8"));
  if (frontmatter === null) {
    fail(plugin, "SKILL.md has no YAML frontmatter");
    return;
  }
  const name = readFrontmatterField(frontmatter, "name");
  if (!name) {
    fail(plugin, "SKILL.md frontmatter has no name field");
    return;
  }
  if (name !== plugin) {
    fail(plugin, `SKILL.md frontmatter name "${name}" does not match plugin "${plugin}"`);
  }
  const description = readFrontmatterField(frontmatter, "description");
  if (!description) fail(plugin, "SKILL.md frontmatter has no description field");
}

// --- Check 4: SKILL.md references resolve to files on disk ---------------
// A SKILL.md path reference is only validated when it is unambiguously a
// reference to *this plugin's own bundle*:
//
//  - Any `${CLAUDE_PLUGIN_ROOT}/...` path — explicitly plugin-root-relative.
//  - A bare `workflows/`, `references/`, or `tools/` path — these subdir
//    names are skill-bundle vocabulary, effectively never used for anything
//    else.
//
// Bare `scripts/`, `cli/`, `assets/`, `hooks/` paths are NOT validated unless
// `${CLAUDE_PLUGIN_ROOT}`-prefixed: those are universal repo conventions and
// appear as illustrative examples (an audit skill citing `./scripts/test.sh`)
// or cross-skill delegation (`scripts/gen.sh` belonging to another skill).
// Bare filenames (e.g. `data-layer.md`) are likewise not validated — too
// easily confused with filenames inside code examples.
const REF_EXT = "md|ts|tsx|sh|json|js|mjs|cjs|py";
const ALL_SUBDIRS = ["workflows", "references", "tools", "scripts", "cli", "assets", "hooks"];
const BARE_OK_SUBDIRS = new Set(["workflows", "references", "tools"]);
const REF_PATTERN = new RegExp(
  `(\\$\\{CLAUDE_PLUGIN_ROOT\\}/)?((?:skills/[a-z0-9-]+/)?(?:${ALL_SUBDIRS.join("|")})/[A-Za-z0-9._/-]+\\.(?:${REF_EXT}))`,
  "g",
);

/** First bundle-subdir component of a reference path (after any skills/<x>/). */
function refSubdir(pathPart) {
  return pathPart.replace(/^skills\/[a-z0-9-]+\//, "").split("/")[0];
}

/** Resolve a SKILL.md reference to an absolute path in the plugin port. */
function resolveRef(plugin, prefixed, pathPart) {
  // `${CLAUDE_PLUGIN_ROOT}/...` is plugin-root-relative; a bare reference is
  // relative to the skill directory (skills/<plugin>/).
  const base = prefixed ? path.join(pluginsDir, plugin) : path.join(pluginsDir, plugin, "skills", plugin);
  return path.join(base, pathPart);
}

function checkSkillReferences(plugin) {
  const file = skillPath(plugin);
  if (!fs.existsSync(file)) return; // already reported by checkSkillFrontmatter
  const text = fs.readFileSync(file, "utf8");
  const seen = new Set();
  for (const [, prefix, pathPart] of text.matchAll(REF_PATTERN)) {
    const prefixed = Boolean(prefix);
    if (!prefixed && !BARE_OK_SUBDIRS.has(refSubdir(pathPart))) continue;
    // biome-ignore lint/suspicious/noTemplateCurlyInString: literal "${CLAUDE_PLUGIN_ROOT}" placeholder text, not a JS template
    const ref = (prefixed ? "${CLAUDE_PLUGIN_ROOT}/" : "") + pathPart;
    if (seen.has(ref)) continue;
    seen.add(ref);
    if (!fs.existsSync(resolveRef(plugin, prefixed, pathPart))) {
      fail(plugin, `SKILL.md references missing file: ${ref}`);
    }
  }
}

// --- Check 5: every plugin has exactly one marketplace.json entry --------
const marketplacePath = path.join(repoRoot, ".claude-plugin", "marketplace.json");

function checkMarketplace(pluginList, manifestDescriptions) {
  let marketplace;
  try {
    marketplace = JSON.parse(fs.readFileSync(marketplacePath, "utf8"));
  } catch (err) {
    fail("(marketplace)", `.claude-plugin/marketplace.json does not parse: ${err.message}`);
    return;
  }
  const entries = marketplace.plugins ?? [];
  const counts = new Map();
  for (const entry of entries) {
    counts.set(entry.name, (counts.get(entry.name) ?? 0) + 1);
    if (!pluginList.includes(entry.name)) {
      fail(entry.name ?? "(unnamed)", "marketplace.json entry has no matching plugin directory");
      continue;
    }
    const expectedSource = `./claude-code/plugins/${entry.name}`;
    if (entry.source !== expectedSource) {
      fail(entry.name, `marketplace.json source "${entry.source}" should be "${expectedSource}"`);
    }
    const expectedDescription = manifestDescriptions.get(entry.name);
    if (expectedDescription && entry.description !== expectedDescription) {
      fail(entry.name, "marketplace.json description does not match plugin.json");
    }
  }
  for (const plugin of pluginList) {
    const count = counts.get(plugin) ?? 0;
    if (count === 0) fail(plugin, "missing marketplace.json entry");
    else if (count > 1) fail(plugin, `has ${count} marketplace.json entries (expected 1)`);
  }
}

// --- Check 6: every plugin has a docs/catalog.md Claude Code Plugin row --
const catalogPath = path.join(repoRoot, "docs", "catalog.md");

function checkCatalog(pluginList) {
  if (!fs.existsSync(catalogPath)) {
    fail("(catalog)", "docs/catalog.md not found");
    return;
  }
  const text = fs.readFileSync(catalogPath, "utf8");
  const listed = new Set();
  for (const m of text.matchAll(/^\|\s*`([^`]+)`\s*\|\s*Claude Code Plugin\s*\|/gm)) {
    listed.add(m[1]);
  }
  for (const plugin of pluginList) {
    if (!listed.has(plugin)) fail(plugin, "missing docs/catalog.md Claude Code Plugin row");
  }
}

// --- Check 7: no junk artifacts under claude-code/plugins/ ---------------
// node_modules is skipped during the walk (it is gitignored, may exist
// locally, and must never be flagged).
const JUNK_FILES = new Set([".DS_Store", "Thumbs.db"]);

function checkJunk() {
  const stack = [pluginsDir];
  while (stack.length > 0) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "node_modules" || entry.name === ".git") continue;
        stack.push(full);
      } else if (JUNK_FILES.has(entry.name)) {
        const rel = path.relative(pluginsDir, full);
        fail(rel.split(path.sep)[0] || "(plugins)", `junk artifact present: ${entry.name}`);
      }
    }
  }
}

// --- Run -----------------------------------------------------------------
const plugins = listPlugins();
const manifestDescriptions = new Map();
for (const plugin of plugins) {
  const description = checkPluginManifest(plugin);
  if (description) manifestDescriptions.set(plugin, description);
  checkReadme(plugin);
  checkSkillFrontmatter(plugin);
  checkSkillReferences(plugin);
}
checkMarketplace(plugins, manifestDescriptions);
checkCatalog(plugins);
checkJunk();

if (failures.length > 0) {
  console.error(`\n✗ plugin-port validation failed (${failures.length} issue(s)):\n`);
  for (const { plugin, message } of failures) {
    console.error(`  [${plugin}] ${message}`);
  }
  console.error("");
  process.exit(1);
}

console.log(`✓ plugin-port validation passed (${plugins.length} plugins)`);
