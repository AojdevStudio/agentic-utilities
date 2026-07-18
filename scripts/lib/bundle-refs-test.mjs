import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { resolveBundleReference } from "./bundle-refs.mjs";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "bundle-refs-"));
const base = path.join(tmp, "skills", "fixture-plugin");
fs.mkdirSync(path.join(base, "scripts"), { recursive: true });
fs.writeFileSync(path.join(base, "scripts", "gen.sh"), "#!/bin/sh\necho ok\n");
fs.mkdirSync(path.join(tmp, "secret"), { recursive: true });
fs.writeFileSync(path.join(tmp, "secret", "outside.md"), "should never be reachable\n");

// ok: reference stays inside base and the file exists.
{
  const result = resolveBundleReference(base, "scripts/gen.sh");
  assert.equal(result.ok, true, "existing in-bundle script must resolve ok");
  assert.equal(result.resolved, path.join(base, "scripts", "gen.sh"));
}

// missing: reference stays inside base but no file exists there.
const MISSING_CASES = ["scripts/does-not-exist.sh", "assets/missing-logo.png", "tools/absent-tool.ts"];
for (const pathPart of MISSING_CASES) {
  const result = resolveBundleReference(base, pathPart);
  assert.equal(result.ok, false, `${pathPart}: expected not ok`);
  assert.equal(result.reason, "missing", `${pathPart}: expected reason "missing"`);
}

// traversal: reference walks out of base, even when it lands on a real file
// elsewhere in the repo (the actual attack this guards against).
const TRAVERSAL_CASES = [
  "scripts/../../secret/outside.md",
  "../../secret/outside.md",
  "scripts/../../../secret/outside.md",
  "scripts/./gen.sh",
];
for (const pathPart of TRAVERSAL_CASES) {
  const result = resolveBundleReference(base, pathPart);
  assert.equal(result.ok, false, `${pathPart}: expected not ok`);
  assert.equal(result.reason, "traversal", `${pathPart}: expected reason "traversal"`);
}

fs.rmSync(tmp, { recursive: true, force: true });

console.log("bundle reference resolution fixtures passed");
