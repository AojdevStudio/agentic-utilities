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

fs.rmSync(tmp, { recursive: true, force: true });

console.log("bundle reference resolution fixtures passed");
