import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { checkReferenceOnDisk } from "../validate-plugin-ports.mjs";
import { extractBundleReferences, resolveBundleReference } from "./bundle-refs.mjs";

const SUBDIRS = ["workflows", "references", "tools", "scripts", "cli", "assets", "hooks"];
const EXTENSIONS = [
  "md",
  "ts",
  "tsx",
  "sh",
  "json",
  "js",
  "mjs",
  "cjs",
  "py",
  "png",
  "jpg",
  "jpeg",
  "gif",
  "svg",
  "webp",
];
function extract(text) {
  return extractBundleReferences(text, { subdirs: SUBDIRS, extensions: EXTENSIONS });
}

// --- extractBundleReferences: token boundaries and asset coverage --------

{
  // A production-realistic image reference must actually be extracted — the
  // prior REF_EXT list excluded image extensions entirely, so this never
  // reached resolveBundleReference at all regardless of what it returned.
  const refs = extract("See `assets/missing-logo.png` for the logo.");
  assert.equal(refs.length, 1);
  assert.deepEqual(refs[0], { prefixed: false, pathPart: "assets/missing-logo.png", ref: "assets/missing-logo.png" });
}

{
  // A reference embedded mid-path, preceded by a word/path character, must
  // not "resync" and match as if it started a fresh token.
  const refs = extract("xtools/outside.ts should never match as a bare reference");
  assert.equal(refs.length, 0, "mid-identifier text must not match");
}

{
  // Terminal boundary: a coincidental prefix of a longer filename must not
  // be truncated into a match for a shorter, unrelated real asset name.
  const refs = extract("See `assets/logo.png.bak` for the backup.");
  assert.equal(refs.length, 0, "assets/logo.png.bak must not extract as assets/logo.png");
}

{
  // A safe leading "./" (same-directory prefix) is a common, harmless
  // convention and must be captured, not silently unmatched.
  const refs = extract("Run `./scripts/gen.sh` now.");
  assert.equal(refs.length, 1, "./scripts/gen.sh must be extracted");
  assert.deepEqual(refs[0], { prefixed: false, pathPart: "./scripts/gen.sh", ref: "./scripts/gen.sh" });
}

{
  // The exact adversarial case from the review: a ${CLAUDE_PLUGIN_ROOT}/../
  // reference must keep its traversal prefix attached to the captured
  // pathPart, not get reinterpreted as the bare tail `tools/outside.ts`.
  // biome-ignore lint/suspicious/noTemplateCurlyInString: literal "${CLAUDE_PLUGIN_ROOT}" placeholder text, not a JS template
  const refs = extract("Run ${CLAUDE_PLUGIN_ROOT}/../tools/outside.ts now.");
  assert.equal(refs.length, 1, "must extract exactly one reference, not a bare-tail duplicate");
  assert.deepEqual(refs[0], {
    prefixed: true,
    pathPart: "../tools/outside.ts",
    // biome-ignore lint/suspicious/noTemplateCurlyInString: literal "${CLAUDE_PLUGIN_ROOT}" placeholder text, not a JS template
    ref: "${CLAUDE_PLUGIN_ROOT}/../tools/outside.ts",
  });
}

// --- resolveBundleReference: pure lexical containment (no filesystem) ----

{
  const result = resolveBundleReference("/tmp/base", "scripts/gen.sh");
  assert.equal(result.ok, true);
  assert.equal(result.resolved, path.join("/tmp/base", "scripts", "gen.sh"));
}

{
  // A lone "." (current-dir) segment is a safe no-op and must be accepted,
  // not rejected as traversal.
  const result = resolveBundleReference("/tmp/base", "scripts/./gen.sh");
  assert.equal(result.ok, true, "scripts/./gen.sh must not be treated as traversal");
}

for (const pathPart of ["scripts/../../secret/outside.md", "../../secret/outside.md", "../tools/outside.ts"]) {
  const result = resolveBundleReference("/tmp/base", pathPart);
  assert.equal(result.ok, false, `${pathPart}: expected not ok`);
  assert.equal(result.reason, "traversal", `${pathPart}: expected reason "traversal"`);
}

// --- checkReferenceOnDisk: filesystem boundary (existence, regular-file,
// symlink containment) lives at the validator, not in the pure lib ---------

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "bundle-refs-"));
const base = path.join(tmp, "skills", "fixture-plugin");
fs.mkdirSync(path.join(base, "scripts"), { recursive: true });
fs.mkdirSync(path.join(base, "assets"), { recursive: true });
// A real file at the shorter, truncated path a terminal-boundary bug would
// coincidentally match against (assets/logo.png.bak -> assets/logo.png).
fs.writeFileSync(path.join(base, "assets", "logo.png"), "not a real png, just fixture bytes\n");
fs.mkdirSync(path.join(base, "tools"), { recursive: true });
fs.writeFileSync(path.join(base, "scripts", "gen.sh"), "#!/bin/sh\necho ok\n");
// A directory whose name matches the reference vocabulary + extension
// pattern (e.g. a stray "tool.ts" directory) must not pass as a file.
fs.mkdirSync(path.join(base, "tools", "tool.ts"), { recursive: true });
// A real file planted at the *bare* traversal-tail location, to prove the
// adversarial same-named-file case: even though tools/outside.ts genuinely
// exists inside the bundle, a reference that actually reads
// ${CLAUDE_PLUGIN_ROOT}/../tools/outside.ts must still be rejected as
// traversal — it must never be matched against this coincidental file.
fs.writeFileSync(path.join(base, "tools", "outside.ts"), "// decoy, must never be reached via traversal\n");
fs.mkdirSync(path.join(tmp, "secret"), { recursive: true });
fs.writeFileSync(path.join(tmp, "secret", "real-target.md"), "should never be reachable\n");
fs.symlinkSync(path.join(tmp, "secret", "real-target.md"), path.join(base, "scripts", "escape-link.md"));
fs.symlinkSync(path.join(base, "scripts", "gen.sh"), path.join(base, "scripts", "safe-link.sh"));
// A symlinked *parent directory* escaping the bundle, with an ordinary
// regular file sitting behind it. lstat on the final component alone can't
// see this: path resolution follows the intermediate symlink transparently,
// so the final component looks like a plain contained file unless the whole
// resolved path is realpath-checked.
fs.mkdirSync(path.join(tmp, "secret", "linked-target"), { recursive: true });
fs.writeFileSync(path.join(tmp, "secret", "linked-target", "payload.md"), "should never be reachable\n");
fs.symlinkSync(path.join(tmp, "secret", "linked-target"), path.join(base, "scripts", "linked-dir"));
// A dangling symlink (target does not exist).
fs.symlinkSync(path.join(base, "scripts", "does-not-exist.sh"), path.join(base, "scripts", "dangling.sh"));
// A cyclic symlink pair.
fs.symlinkSync(path.join(base, "scripts", "cycle-b"), path.join(base, "scripts", "cycle-a"));
fs.symlinkSync(path.join(base, "scripts", "cycle-a"), path.join(base, "scripts", "cycle-b"));

{
  // missing: lexically contained, no file on disk.
  const resolved = path.join(base, "assets", "missing-logo.png");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "missing");
}

{
  // not-a-file: a directory sitting at the exact reference path.
  const resolved = path.join(base, "tools", "tool.ts");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "not-a-file");
}

{
  // symlink-escape: the reference's own path is inside the bundle, but it
  // points at a real target outside it.
  const resolved = path.join(base, "scripts", "escape-link.md");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "symlink-escape");
}

{
  // A symlink whose target stays inside the bundle is a legitimate file.
  const resolved = path.join(base, "scripts", "safe-link.sh");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "ok");
}

{
  // A genuine in-bundle file.
  const resolved = path.join(base, "scripts", "gen.sh");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "ok");
}

{
  // The exact adversarial case from the review: an escaping *parent*
  // directory symlink, reached through a final component that is itself a
  // plain regular file (not a symlink).
  const resolved = path.join(base, "scripts", "linked-dir", "payload.md");
  assert.equal(
    checkReferenceOnDisk(base, resolved).reason,
    "symlink-escape",
    "a file behind a symlinked parent directory must be rejected as escaping",
  );
}

{
  // A dangling symlink must fail as a controlled validation reason, not
  // throw an uncaught ENOENT out of realpathSync.
  const resolved = path.join(base, "scripts", "dangling.sh");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "missing");
}

{
  // A cyclic symlink pair must fail as a controlled validation reason, not
  // throw an uncaught ELOOP out of realpathSync.
  const resolved = path.join(base, "scripts", "cycle-a");
  assert.equal(checkReferenceOnDisk(base, resolved).reason, "missing");
}

// --- End-to-end: extractor -> resolver -> on-disk check, chained exactly
// the way validate-plugin-ports.mjs's checkSkillReferences runs them -------

function validateReference(skillMdText, pluginBase) {
  const [entry] = extract(skillMdText);
  assert.ok(entry, "expected exactly one extracted reference");
  const lexical = resolveBundleReference(pluginBase, entry.pathPart);
  if (!lexical.ok) return lexical.reason;
  return checkReferenceOnDisk(pluginBase, lexical.resolved).reason;
}

assert.equal(
  validateReference("See `assets/missing-logo.png` for the logo.", base),
  "missing",
  "production pipeline must flag a missing PNG asset reference",
);

assert.equal(
  // biome-ignore lint/suspicious/noTemplateCurlyInString: literal "${CLAUDE_PLUGIN_ROOT}" placeholder text, not a JS template
  validateReference("Run ${CLAUDE_PLUGIN_ROOT}/../tools/outside.ts now.", base),
  "traversal",
  "production pipeline must reject the traversal reference even though tools/outside.ts genuinely exists in the bundle",
);

assert.equal(
  validateReference("Run `./scripts/gen.sh` now.", base),
  "ok",
  "production pipeline must accept a safe leading ./ prefix",
);

{
  // The exact terminal-boundary adversarial case, end to end: even with a
  // real file sitting at the truncated path (assets/logo.png), the
  // coincidental-prefix text must never extract a match to validate at all.
  const refs = extract("See `assets/logo.png.bak` for the backup.");
  assert.equal(
    refs.length,
    0,
    "production pipeline must never match assets/logo.png.bak against the real assets/logo.png file",
  );
}

fs.rmSync(tmp, { recursive: true, force: true });

console.log("bundle reference resolution fixtures passed");
