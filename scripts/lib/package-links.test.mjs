import { test } from "bun:test";
import assert from "node:assert/strict";
import { findBrokenPackedLinks } from "./package-links.mjs";

const readTarget = () => "# Included target\n";

test("checkout-only targets fail packed-link validation", () => {
  const broken = findBrokenPackedLinks({
    sourcePath: "skills/herdr-fleet/README.md",
    content: "[private](private.md)",
    packedPathSet: new Set(["skills/herdr-fleet/README.md"]),
    readTarget,
  });
  assert.deepEqual(broken, ["skills/herdr-fleet/README.md -> private.md"]);
});

test("packed targets with valid anchors pass", () => {
  const broken = findBrokenPackedLinks({
    sourcePath: "skills/herdr-fleet/README.md",
    content: "[included](included.md#included-target)",
    packedPathSet: new Set(["skills/herdr-fleet/README.md", "skills/herdr-fleet/included.md"]),
    readTarget,
  });
  assert.deepEqual(broken, []);
});

test("link destinations ignore surrounding whitespace and optional titles", () => {
  const broken = findBrokenPackedLinks({
    sourcePath: "skills/herdr-fleet/README.md",
    content: '[included]( included.md#included-target "Title" ) [external]( https://example.com "Title" )',
    packedPathSet: new Set(["skills/herdr-fleet/README.md", "skills/herdr-fleet/included.md"]),
    readTarget,
  });
  assert.deepEqual(broken, []);
});

test("whitespace-prefixed missing targets remain broken after title removal", () => {
  const broken = findBrokenPackedLinks({
    sourcePath: "skills/herdr-fleet/README.md",
    content: '[missing]( missing.md "Title" )',
    packedPathSet: new Set(["skills/herdr-fleet/README.md"]),
    readTarget,
  });
  assert.deepEqual(broken, ["skills/herdr-fleet/README.md -> missing.md"]);
});
