import { test } from "bun:test";
import assert from "node:assert/strict";
import { dependencyMetadataMismatches } from "./validate-lockfile.mjs";

const manifest = {
  dependencies: { runtime: "^1.0.0" },
  devDependencies: { compiler: "latest" },
  peerDependencies: { host: "*" },
};

test("matching root dependency metadata passes", () => {
  const lockfile = { packages: { "": structuredClone(manifest) } };
  assert.deepEqual(dependencyMetadataMismatches(manifest, lockfile), []);
});

test("dependency range drift reports the exact field", () => {
  const lockfile = { packages: { "": { ...structuredClone(manifest), devDependencies: { compiler: "*" } } } };
  assert.deepEqual(dependencyMetadataMismatches(manifest, lockfile), [
    'devDependencies.compiler: package.json="latest", package-lock.json="*"',
  ]);
});
