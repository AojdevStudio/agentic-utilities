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
import { extractBundleReferences, resolveBundleReference } from "./lib/bundle-refs.mjs";
import { isNonEmptyString, parseFrontmatter } from "./lib/frontmatter.mjs";

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
  let frontmatter;
  try {
    frontmatter = parseFrontmatter(fs.readFileSync(file, "utf8"), file);
  } catch (err) {
    fail(plugin, err.message);
    return;
  }
  const name = frontmatter.name;
  if (!isNonEmptyString(name)) {
    fail(plugin, "SKILL.md frontmatter has no name field");
    return;
  }
  if (name !== plugin) {
    fail(plugin, `SKILL.md frontmatter name "${name}" does not match plugin "${plugin}"`);
  }
  if (!isNonEmptyString(frontmatter.description)) {
    fail(plugin, "SKILL.md frontmatter has no description field");
  }
}

// --- Check 4: SKILL.md references resolve to files on disk ---------------
// A SKILL.md path reference is only matched when it is unambiguously a
// reference to *this plugin's own bundle*: either `${CLAUDE_PLUGIN_ROOT}/...`
// (explicitly plugin-root-relative) or a bare `workflows/`, `references/`,
// `tools/`, `scripts/`, `cli/`, `assets/`, or `hooks/` path — this fixed
// vocabulary of skill-bundle subdirectory names is what makes the match
// unambiguous. Every match is validated: it must resolve to an existing
// *regular file* whose real (symlink-resolved) path stays inside the
// plugin's own root — rejects `..` traversal, directories masquerading as
// files, and symlinks that escape the bundle.
const REF_EXT = ["md", "ts", "tsx", "sh", "json", "js", "mjs", "cjs", "py", "png", "jpg", "jpeg", "gif", "svg", "webp"];
const ALL_SUBDIRS = ["workflows", "references", "tools", "scripts", "cli", "assets", "hooks"];

/**
 * Filesystem-facing half of reference validation, applied only after
 * `resolveBundleReference` has already confirmed `resolved` is lexically
 * inside `base`. `realpathSync` is required unconditionally (not just when
 * the final path component is itself a symlink) because a symlinked
 * *parent* directory resolves away silently otherwise: `lstatSync` on the
 * final component follows all intermediate symlinks as a normal part of
 * path resolution, so a plain file sitting behind a symlinked directory
 * looks like an ordinary contained file unless the whole path is
 * realpath-resolved and re-checked for containment. A dangling or cyclic
 * symlink makes `realpathSync` throw (ENOENT / ELOOP); that is a controlled
 * "missing" result here, not an uncaught crash.
 */
export function checkReferenceOnDisk(base, resolved) {
  let real;
  try {
    real = fs.realpathSync(resolved);
  } catch {
    return { reason: "missing" };
  }
  const realBase = fs.realpathSync(base);
  const rel = path.relative(realBase, real);
  if (rel.startsWith("..") || path.isAbsolute(rel)) {
    return { reason: "symlink-escape" };
  }
  if (!fs.statSync(real).isFile()) {
    return { reason: "not-a-file" };
  }
  return { reason: "ok" };
}

const ON_DISK_MESSAGES = {
  missing: (ref) => `SKILL.md references missing file: ${ref}`,
  "symlink-escape": (ref) => `SKILL.md reference resolves outside the plugin bundle via symlink: ${ref}`,
  "not-a-file": (ref) => `SKILL.md reference is not a regular file: ${ref}`,
};

function checkSkillReferences(plugin) {
  const file = skillPath(plugin);
  if (!fs.existsSync(file)) return; // already reported by checkSkillFrontmatter
  const text = fs.readFileSync(file, "utf8");
  const refs = extractBundleReferences(text, { subdirs: ALL_SUBDIRS, extensions: REF_EXT });
  for (const { prefixed, pathPart, ref } of refs) {
    // `${CLAUDE_PLUGIN_ROOT}/...` is plugin-root-relative; a bare reference
    // is relative to the skill directory (skills/<plugin>/).
    const base = prefixed ? path.join(pluginsDir, plugin) : path.join(pluginsDir, plugin, "skills", plugin);
    const lexical = resolveBundleReference(base, pathPart);
    if (!lexical.ok) {
      fail(plugin, `SKILL.md reference escapes the plugin bundle: ${ref}`);
      continue;
    }
    const { reason } = checkReferenceOnDisk(base, lexical.resolved);
    if (reason !== "ok") fail(plugin, ON_DISK_MESSAGES[reason](ref));
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
// Guarded so a test can `import { checkReferenceOnDisk } from "./validate-plugin-ports.mjs"`
// without triggering a full validation run against the real repository.
function main() {
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
}

// A plain `import.meta.url === \`file://${process.argv[1]}\`` string
// comparison is not URL-safe: import.meta.url percent-encodes characters
// like spaces, while the raw argv path does not, so a checkout path
// containing a space silently fails the check and main() never runs.
// Comparing filesystem paths instead of URL strings sidesteps encoding
// entirely: fileURLToPath decodes import.meta.url back to a native path
// (already realpath-resolved by Node for a directly-executed script — e.g.
// through macOS's /tmp -> /private/tmp symlink), and realpathSync resolves
// the same way on the argv side so both sides compare on equal footing
// regardless of spaces, separators, or symlinked ancestor directories.
function isDirectExecution() {
  if (!process.argv[1]) return false;
  try {
    return fileURLToPath(import.meta.url) === fs.realpathSync(process.argv[1]);
  } catch {
    return false;
  }
}

if (isDirectExecution()) main();
