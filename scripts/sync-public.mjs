#!/usr/bin/env node
/**
 * Generate this repo's public skills from the private skill store.
 *
 * The store is canonical for `mirror` skills; everything under `skills/` that a
 * manifest entry claims is a build artifact. Private detail is removed by
 * declared transforms, and anything the transforms miss is caught by the leak
 * assertion — a transform gap fails the run instead of shipping.
 *
 *   bun scripts/sync-public.mjs            # write
 *   bun scripts/sync-public.mjs --check    # report drift, write nothing (exit 1 on drift)
 *   bun scripts/sync-public.mjs --prune    # also delete orphaned generated files
 *   bun scripts/sync-public.mjs --skill find-docs
 *
 * See docs/public-sync.md.
 */
import { mkdir, readdir, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, sep } from "node:path";
import { parseDocument } from "yaml";

// ---- pure helpers (exported for scripts/sync-public.test.mjs) ---------------

/** Expand a leading `~/` against the current user's home directory. */
export function expandHome(inputPath, home = homedir()) {
  if (inputPath === "~") return home;
  if (inputPath.startsWith("~/")) return join(home, inputPath.slice(2));
  return inputPath;
}

/**
 * Translate a manifest glob into an anchored regex.
 * `**` crosses path separators, `*` does not. Everything else is literal.
 */
export function globToRegExp(glob) {
  let out = "";
  for (let i = 0; i < glob.length; i += 1) {
    const char = glob[i];
    if (char === "*") {
      if (glob[i + 1] === "*") {
        out += ".*";
        i += 1;
      } else {
        out += "[^/]*";
      }
      continue;
    }
    out += char.replace(/[.+?^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${out}$`);
}

/** True when `relPath` matches any glob in `globs`. */
export function matchesAnyGlob(relPath, globs = []) {
  return globs.some((glob) => globToRegExp(glob).test(relPath));
}

/**
 * Delete dotted frontmatter keys (e.g. `metadata.machines`) from a document's
 * YAML frontmatter, dropping a parent mapping that the deletion left empty.
 * Files without frontmatter pass through untouched — most skill files are prose.
 */
export function dropFrontmatterKeys(content, dottedKeys) {
  const normalized = content.replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---\n")) return content;
  const match = /^---\n([\s\S]*?)\n---[ \t]*(?:\n|$)/.exec(normalized);
  if (!match) return content;

  const doc = parseDocument(match[1]);
  let changed = false;
  for (const dotted of dottedKeys) {
    const path = dotted.split(".");
    if (!doc.hasIn(path)) continue;
    doc.deleteIn(path);
    changed = true;
    // A parent left with no keys is noise; remove it rather than emit `metadata: {}`.
    if (path.length > 1) {
      const parentPath = path.slice(0, -1);
      const parent = doc.getIn(parentPath);
      if (parent && typeof parent.items?.length === "number" && parent.items.length === 0) {
        doc.deleteIn(parentPath);
      }
    }
  }
  if (!changed) return content;

  const body = normalized.slice(match[0].length);
  // Match hand-written frontmatter conventions: no padding inside flow
  // collections, no line wrapping. Without these the serializer rewrites
  // `[claude, codex]` and long descriptions on every run, so a content-free
  // sync would still report drift.
  const yaml = doc.toString({ flowCollectionPadding: false, lineWidth: 0 }).replace(/\n$/, "");
  return `---\n${yaml}\n---\n${body}`;
}

/**
 * Force the frontmatter `name:` to the public directory name. The public repo
 * validates that name and directory agree, so a renamed skill (`git-workflow`
 * in the store, `gitworkflow` here) must be rewritten, not copied.
 */
export function rewriteSkillName(content, publicName) {
  const normalized = content.replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---\n")) return content;
  const match = /^---\n([\s\S]*?)\n---[ \t]*(?:\n|$)/.exec(normalized);
  if (!match) return content;
  const rewritten = match[1].replace(/^name:[ \t]*.*$/m, `name: ${publicName}`);
  if (rewritten === match[1]) return content;
  return `---\n${rewritten}\n---\n${normalized.slice(match[0].length)}`;
}

/**
 * Merge the gitignored local overlay into the tracked manifest.
 *
 * The tracked file holds structural patterns only. Everything that names a
 * private machine, repo, or domain lives in the overlay, because publishing
 * that list would leak exactly what the list exists to protect. Overlay leak
 * patterns are appended; overlay replacements run after the tracked ones.
 */
export function mergeLocalOverlay(manifest, overlay) {
  if (!overlay) return manifest;
  return {
    ...manifest,
    leakPatterns: [...(manifest.leakPatterns ?? []), ...(overlay.leakPatterns ?? [])],
    transforms: {
      ...manifest.transforms,
      replace: [...(manifest.transforms?.replace ?? []), ...(overlay.transforms?.replace ?? [])],
    },
  };
}

/** Apply declared literal replacements in manifest order. */
export function applyReplacements(content, rules = []) {
  return rules.reduce((acc, rule) => acc.split(rule.find).join(rule.replaceWith), content);
}

/** Drop whole lines matching any of the given regex sources. */
export function dropLines(content, patterns = []) {
  if (patterns.length === 0) return content;
  const regexes = patterns.map((source) => new RegExp(source));
  const kept = content.split("\n").filter((line) => !regexes.some((regex) => regex.test(line)));
  return kept.join("\n");
}

/**
 * Run the full transform chain for one text file. Per-skill rules run after the
 * global ones so a skill can refine, not fight, the shared pass.
 */
export function transformText(content, { publicName, transforms, skillRules = {}, isSkillManifest }) {
  let out = content;
  if (isSkillManifest) {
    out = dropFrontmatterKeys(out, transforms.dropFrontmatterKeys ?? []);
    out = rewriteSkillName(out, publicName);
  }
  out = applyReplacements(out, transforms.replace ?? []);
  out = applyReplacements(out, skillRules.replace ?? []);
  return dropLines(out, skillRules.dropLines ?? []);
}

/**
 * Find published text that still points at a file the manifest excluded.
 *
 * This deliberately asks the narrow question rather than "does every path-shaped
 * string resolve". Skills legitimately name paths in the repo they operate on
 * (`scripts/test.sh`, `.github/workflows/ci.yml`), and treating those as bundle
 * references produced far more noise than signal. What actually breaks a
 * published skill is an exclusion that left a pointer behind, so that is what
 * this checks.
 *
 * `excludedPaths` are source-relative paths that matched an `exclude` glob, and
 * only those exact paths are searched for. Directory prefixes were tried and
 * dropped: a skill that documents house conventions ("tools -> `tools/<name>`")
 * matches `tools/` without referring to the bundle at all.
 */
export function findExcludedRefs(content, excludedPaths) {
  return [...new Set(excludedPaths)].filter((relPath) => content.includes(relPath)).sort();
}

/**
 * Scan transformed content for private detail the transforms failed to remove.
 * Returns one finding per matching line so the report can point at a location.
 */
export function findLeaks(content, patterns) {
  const findings = [];
  const lines = content.split("\n");
  for (const pattern of patterns) {
    const regex = new RegExp(pattern.regex, "g");
    lines.forEach((line, index) => {
      regex.lastIndex = 0;
      const match = regex.exec(line);
      if (!match) return;
      findings.push({ patternId: pattern.id, line: index + 1, excerpt: match[0] });
    });
  }
  return findings;
}

// ---- filesystem ------------------------------------------------------------

const TEXT_DEFAULTS = [".md", ".txt"];

function isTextFile(relPath, extensions) {
  return (extensions ?? TEXT_DEFAULTS).some((ext) => relPath.endsWith(ext));
}

/** Recursively list files under `dir` as paths relative to it. Missing dir -> []. */
async function listFiles(dir, prefix = "") {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }
  const out = [];
  for (const entry of entries) {
    if (entry.name === ".DS_Store" || entry.name === "node_modules" || entry.name === ".git") continue;
    const relPath = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...(await listFiles(join(dir, entry.name), relPath)));
    else out.push(relPath);
  }
  return out.sort();
}

async function pathExists(target) {
  try {
    await stat(target);
    return true;
  } catch {
    return false;
  }
}

/**
 * Refuse to run when a store entry resolves back inside this repo. That is the
 * old symlink arrangement, where source and destination are the same bytes and
 * "syncing" would silently mean "no scrub ever happened".
 */
async function assertNotCircular(storeDir, repoRoot, publicName) {
  const resolved = await realpath(storeDir);
  const root = await realpath(repoRoot);
  if (resolved === root || resolved.startsWith(root + sep)) {
    throw new Error(
      `${publicName}: store entry resolves inside this repo (${resolved}).\n` +
        `  It is still a symlink into the public checkout, so no scrub can run.\n` +
        `  Materialize it in the store first, then re-run.`,
    );
  }
}

/** Build the write plan for one mirrored skill without touching disk. */
async function planSkill({ publicName, entry, storeRoot, publicSkillsDir, manifest, repoRoot }) {
  const sourceName = entry.source ?? publicName;
  const storeDir = join(storeRoot, sourceName);
  const publicDir = join(publicSkillsDir, publicName);

  if (!(await pathExists(storeDir))) {
    return { publicName, status: "missing-source", detail: storeDir, writes: [], orphans: [], leaks: [] };
  }
  await assertNotCircular(storeDir, repoRoot, publicName);

  const sourceFiles = await listFiles(storeDir);
  const writes = [];
  const leaks = [];

  const excludedPaths = sourceFiles.filter((relPath) => matchesAnyGlob(relPath, entry.exclude));

  for (const relPath of sourceFiles) {
    if (matchesAnyGlob(relPath, entry.exclude)) continue;
    if (matchesAnyGlob(relPath, entry.publicOwned)) continue;

    const absSource = join(storeDir, relPath);
    const absTarget = join(publicDir, relPath);

    if (!isTextFile(relPath, manifest.textExtensions)) {
      writes.push({ relPath, absTarget, content: await readFile(absSource), binary: true });
      continue;
    }

    const raw = await readFile(absSource, "utf8");
    const content = transformText(raw, {
      publicName,
      transforms: manifest.transforms,
      skillRules: entry,
      isSkillManifest: relPath === "SKILL.md",
    });

    for (const finding of findLeaks(content, manifest.leakPatterns)) {
      leaks.push({ file: `${publicName}/${relPath}`, ...finding });
    }
    writes.push({ relPath, absTarget, content, binary: false });
  }

  const generated = new Set(writes.map((write) => write.relPath));
  const existing = await listFiles(publicDir);
  const orphans = existing.filter(
    (relPath) =>
      !generated.has(relPath) && !matchesAnyGlob(relPath, entry.publicOwned) && !matchesAnyGlob(relPath, entry.exclude),
  );

  const dangling = [];
  for (const write of writes) {
    if (write.binary) continue;
    for (const ref of findExcludedRefs(write.content, excludedPaths)) {
      dangling.push({ file: `${publicName}/${write.relPath}`, ref });
    }
  }

  return { publicName, status: "planned", writes, orphans, leaks, dangling };
}

/** Compare a planned write against what is already on disk. */
async function isDrifted(write) {
  try {
    const existing = await readFile(write.absTarget, write.binary ? undefined : "utf8");
    return write.binary ? !existing.equals(write.content) : existing !== write.content;
  } catch (error) {
    if (error?.code === "ENOENT") return true;
    throw error;
  }
}

// ---- CLI -------------------------------------------------------------------

const GREEN = "✓";
const RED = "✗";
const DOT = "·";

async function main() {
  const args = process.argv.slice(2);
  const checkOnly = args.includes("--check");
  const prune = args.includes("--prune");
  const skillFilterIndex = args.indexOf("--skill");
  const skillFilter = skillFilterIndex === -1 ? null : args[skillFilterIndex + 1];

  const repoRoot = process.cwd();
  const manifestPath = join(repoRoot, "public-manifest.json");
  const tracked = JSON.parse(await readFile(manifestPath, "utf8"));

  // Without the overlay the scrub is strictly weaker than intended, and a
  // weaker scrub that runs silently is worse than one that refuses.
  const overlayPath = join(repoRoot, tracked.localOverlay ?? "public-manifest.local.json");
  let overlay = null;
  if (await pathExists(overlayPath)) {
    overlay = JSON.parse(await readFile(overlayPath, "utf8"));
  } else {
    console.error(`Missing local overlay: ${overlayPath}`);
    console.error("It carries the private-term leak patterns; without it the scrub only covers");
    console.error("structural patterns. Copy public-manifest.local.json.example and fill it in.");
    process.exit(1);
  }
  const manifest = mergeLocalOverlay(tracked, overlay);

  const storeRoot = expandHome(process.env[manifest.storeRootEnv] ?? manifest.storeRoot);
  const publicSkillsDir = join(repoRoot, manifest.publicSkillsDir);

  if (!(await pathExists(storeRoot))) {
    console.log(`Skill store not found at ${storeRoot}.`);
    console.log(`Set ${manifest.storeRootEnv} to point at it. Drift was NOT verified.`);
    process.exit(checkOnly ? 0 : 1);
  }

  const entries = Object.entries(manifest.skills).filter(([name]) => !skillFilter || name === skillFilter);
  if (skillFilter && entries.length === 0) {
    console.error(`Unknown skill '${skillFilter}'. Known: ${Object.keys(manifest.skills).join(", ")}`);
    process.exit(1);
  }

  const drifted = [];
  const allLeaks = [];
  const allOrphans = [];
  const allDangling = [];
  let missingSource = 0;

  for (const [publicName, entry] of entries) {
    if (entry.mode === "public-owned" || entry.mode === "forked") {
      console.log(
        `  ${DOT} ${publicName.padEnd(20)} ${entry.mode}, skipped${entry.reason ? ` (${entry.reason})` : ""}`,
      );
      continue;
    }
    if (entry.mode !== "mirror") {
      console.error(`  ${RED} ${publicName.padEnd(20)} unknown mode '${entry.mode}'`);
      process.exit(1);
    }

    const plan = await planSkill({ publicName, entry, storeRoot, publicSkillsDir, manifest, repoRoot });

    if (plan.status === "missing-source") {
      console.error(`  ${RED} ${publicName.padEnd(20)} no source in store (${plan.detail})`);
      missingSource += 1;
      continue;
    }

    if (plan.leaks.length > 0) {
      allLeaks.push(...plan.leaks);
      console.error(`  ${RED} ${publicName.padEnd(20)} ${plan.leaks.length} leak finding(s)`);
      continue;
    }

    if (plan.dangling.length > 0) {
      allDangling.push(...plan.dangling);
      console.error(`  ${RED} ${publicName.padEnd(20)} ${plan.dangling.length} dangling reference(s)`);
      continue;
    }

    const changed = [];
    for (const write of plan.writes) if (await isDrifted(write)) changed.push(write);

    if (plan.orphans.length > 0) allOrphans.push(...plan.orphans.map((relPath) => ({ publicName, relPath })));

    if (changed.length === 0 && plan.orphans.length === 0) {
      console.log(`  ${GREEN} ${publicName.padEnd(20)} up to date (${plan.writes.length} files)`);
      continue;
    }

    drifted.push(publicName);
    const summary = [
      changed.length > 0 ? `${changed.length} changed` : null,
      plan.orphans.length > 0 ? `${plan.orphans.length} orphaned` : null,
    ]
      .filter(Boolean)
      .join(", ");
    console.log(`  ${RED} ${publicName.padEnd(20)} ${summary}`);
    for (const write of changed) console.log(`      ~ ${write.relPath}`);
    for (const relPath of plan.orphans) console.log(`      ? ${relPath} (only in public)`);

    if (checkOnly) continue;

    for (const write of changed) {
      await mkdir(dirname(write.absTarget), { recursive: true });
      await writeFile(write.absTarget, write.content);
    }
    if (prune) {
      for (const relPath of plan.orphans) {
        await rm(join(publicSkillsDir, publicName, relPath));
        console.log(`      - removed ${relPath}`);
      }
    }
  }

  console.log("");

  if (allLeaks.length > 0) {
    console.error("Leak assertion failed. Nothing was written for the affected skills.\n");
    for (const leak of allLeaks) {
      console.error(`- ${leak.file}:${leak.line} [${leak.patternId}] ${leak.excerpt}`);
    }
    console.error("\nFix the source, add a transform, or exclude the file in public-manifest.json.");
    process.exit(1);
  }

  if (allDangling.length > 0) {
    console.error("Dangling references. Nothing was written for the affected skills.\n");
    for (const item of allDangling)
      console.error(`- ${item.file} points at ${item.ref}, which the published skill does not ship`);
    console.error("\nEither publish the target, or drop the reference with a per-skill dropLines/replace rule.");
    process.exit(1);
  }

  if (missingSource > 0) process.exit(1);

  if (drifted.length === 0) {
    console.log("Public skills match the store.");
    return;
  }

  if (checkOnly) {
    console.error(`${drifted.length} skill(s) drifted: ${drifted.join(", ")}`);
    console.error("Run without --check to sync.");
    process.exit(1);
  }

  console.log(`Synced ${drifted.length} skill(s): ${drifted.join(", ")}`);
  if (allOrphans.length > 0 && !prune) {
    console.log(`${allOrphans.length} orphaned file(s) left in place. Re-run with --prune to delete them.`);
  }
}

// Only run the CLI when executed directly, so the test file can import helpers.
if (import.meta.main !== false && process.argv[1]?.endsWith("sync-public.mjs")) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
