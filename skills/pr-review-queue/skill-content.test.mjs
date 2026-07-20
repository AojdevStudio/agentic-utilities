import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

const skillPath = fileURLToPath(new URL("./SKILL.md", import.meta.url));
const skill = await Bun.file(skillPath).text();

test("every fenced code block declares a language", () => {
  const openers = skill.match(/^```.*$/gm) ?? [];
  const fenceCount = openers.length;
  assert.equal(fenceCount % 2, 0, "fences must come in open/close pairs");
  const openingFences = openers.filter((_, index) => index % 2 === 0);
  for (const fence of openingFences) {
    assert.notEqual(fence, "```", `bare fence with no language: ${fence}`);
  }
});

test("the VERDICT template pins a reviewed head and a sync-mode field", () => {
  assert.match(skill, /VERDICT: MERGE_READY \| NEEDS_WORK \| BLOCKED/);
  assert.match(skill, /^HEAD: <sha>$/m, "VERDICT template must include a machine-readable HEAD field");
  assert.match(
    skill,
    /^SYNC: full-review \| mechanical-delta-ack$/m,
    "VERDICT template must include a machine-readable SYNC field",
  );
});

test("claims are bound to a captured headRefOid, not comment timing", () => {
  assert.match(skill, /head_sha=\$\(gh pr view N --json headRefOid --jq \.headRefOid\)/);
  assert.match(skill, /claim head=\$head_sha/, "claim marker must embed the captured head SHA");
  assert.match(
    skill,
    /re-read to confirm you actually won the claim/i,
    "claim flow must re-read after posting to detect a losing race",
  );
});

test("the open queue is fetched without the default 30-item truncation", () => {
  assert.match(skill, /gh pr list --state open --json number,title,isDraft,createdAt --limit \d{3,}/);
});

test("fleet mode polls instead of terminating on an empty queue", () => {
  assert.match(skill, /Fleet mode \(standing reviewer lane\).*do not terminate/s);
  assert.match(skill, /backoff/i);
  assert.match(skill, /HEARTBEAT/, "idle polling must emit a heartbeat, not a PR comment");
});
