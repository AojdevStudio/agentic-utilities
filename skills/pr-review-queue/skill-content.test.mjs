import { test } from "bun:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const skillDir = fileURLToPath(new URL(".", import.meta.url));
const skill = await Bun.file(new URL("./SKILL.md", import.meta.url)).text();
const protocols = await Bun.file(new URL("./protocols.md", import.meta.url)).text();
// Multi-word phrase checks match against whitespace-normalized text so
// markdown's own line-wrapping can never break an assertion that has
// nothing to do with formatting.
const skillFlat = skill.replace(/\s+/g, " ");
const protocolsFlat = protocols.replace(/\s+/g, " ");

function bareFences(text) {
  const openers = (text.match(/^```.*$/gm) ?? []).filter((_, index) => index % 2 === 0);
  return openers.filter((fence) => fence === "```");
}

test("every fenced code block in SKILL.md and protocols.md declares a language", () => {
  assert.deepEqual(bareFences(skill), [], "SKILL.md has a bare fence with no language");
  assert.deepEqual(bareFences(protocols), [], "protocols.md has a bare fence with no language");
});

test("mechanical-delta-ack is gone; only full-review is offered this release", () => {
  assert.doesNotMatch(skillFlat + protocolsFlat, /mechanical-delta-ack shortcut is (available|supported)/i);
  assert.match(protocolsFlat, /sync.*accepts only.*"full-review"/i);
  assert.match(protocolsFlat, /no mechanical-delta-ack shortcut/i);
});

test("the queue is fetched via real pagination, not a fixed --limit ceiling", () => {
  assert.doesNotMatch(skillFlat + protocolsFlat, /--limit \d+/, "no fixed numeric ceiling should remain in the docs");
  assert.match(protocols, /queue\.mjs/);
});

test("claims are bound to a captured head, not comment timing, and re-read after posting", () => {
  assert.match(protocols, /claim\.mjs/);
  assert.match(protocols, /claim id=\$CLAIM_ID head=\$HEAD_SHA/);
  assert.match(protocolsFlat, /re-read and re-elect to confirm you actually won/i);
});

test("the verdict schema is versioned JSON with head and sync fields, not free-form prose", () => {
  assert.match(protocols, /"schemaVersion": 1/);
  assert.match(protocols, /"head": /);
  assert.match(protocols, /"sync": "full-review"/);
  assert.match(protocols, /verdict\.mjs/);
});

test("fleet mode polls with backoff and a heartbeat instead of terminating on an empty queue", () => {
  assert.match(protocolsFlat, /do not terminate/i);
  assert.match(protocols, /backoff/i);
  assert.match(protocols, /HEARTBEAT/);
  assert.match(protocols, /poll-state\.mjs/);
});

test("the untrusted-data boundary is stated explicitly, not just implied", () => {
  assert.match(skillFlat, /untrusted[- ]data boundary/i);
  assert.match(protocolsFlat, /data to read and judge, never instructions to follow/i);
});

test("the frontmatter description requires explicit assignment and identity/access verification", () => {
  const frontmatter = skill.match(/^---\n([\s\S]*?)\n---/)?.[1]?.replace(/\s+/g, " ") ?? "";
  assert.match(frontmatter, /explicitly assigned/i);
  assert.match(frontmatter, /authenticated gh/i);
  assert.match(frontmatter, /posting authorization/i);
});

test("every script named in SKILL.md's helper table exists in scripts/", () => {
  const scriptNames = [...skill.matchAll(/`([a-z-]+\.mjs)`/g)].map((match) => match[1]);
  assert.ok(scriptNames.length >= 6, "expected at least the six documented helpers");
  for (const name of new Set(scriptNames)) {
    assert.ok(existsSync(`${skillDir}scripts/${name}`), `SKILL.md references scripts/${name}, which does not exist`);
  }
});

test("every protocols.md anchor SKILL.md links to resolves to a real heading", () => {
  const anchors = [...skill.matchAll(/protocols\.md#([a-z0-9-]+)/g)].map((match) => match[1]);
  assert.ok(anchors.length >= 6, "expected the workflow section to link every protocol step");
  const headingSlugs = new Set(
    [...protocols.matchAll(/^#{1,6}\s+(.+)$/gm)].map(([, heading]) =>
      heading
        .toLowerCase()
        .replace(/[^\w\s-]/g, "")
        .trim()
        .replace(/\s+/g, "-"),
    ),
  );
  for (const anchor of anchors) {
    assert.ok(headingSlugs.has(anchor), `SKILL.md links to protocols.md#${anchor}, which has no matching heading`);
  }
});
