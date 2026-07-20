import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  isStaleVerdict,
  parseVerdict,
  VERDICT_SCHEMA_VERSION,
  validateGateEvidence,
  validateVerdict,
} from "./verdict.mjs";

function baseGateEvidence(headSha) {
  return {
    schemaVersion: 1,
    headSha,
    checkedAt: "2026-07-20T00:00:00Z",
    threads: [],
    blockingThreadIds: [],
    humanBlockingThreadIds: [],
    automatedBlockingThreadIds: [],
    checks: [{ name: "lint", state: "pass", required: true, synthesized: false }],
    overall: "pass",
  };
}

function baseVerdict(overrides = {}) {
  const head = overrides.head ?? "bda922abe9b44f9638acf6587ec68fc2bc3945fc";
  return {
    schemaVersion: VERDICT_SCHEMA_VERSION,
    repository: "AojdevStudio/agentic-utilities",
    pullRequest: 35,
    head,
    status: "MERGE_READY",
    sync: "full-review",
    gates: baseGateEvidence(head),
    blocking: [],
    nonBlocking: [],
    timestamp: "2026-07-20T00:00:00Z",
    ...overrides,
  };
}

// --- valid verdicts, one per status ---

test("a valid MERGE_READY verdict has no errors", () => {
  assert.deepEqual(validateVerdict(baseVerdict()), []);
});

test("a valid NEEDS_WORK verdict requires fixScope and at least one blocking ID", () => {
  const verdict = baseVerdict({
    status: "NEEDS_WORK",
    blocking: ["F1"],
    fixScope: "Fix the claim election tiebreak.",
  });
  assert.deepEqual(validateVerdict(verdict), []);
});

test("a valid BLOCKED verdict requires blockedReason", () => {
  const verdict = baseVerdict({ status: "BLOCKED", blockedReason: "CI provider outage, unrelated to the diff." });
  assert.deepEqual(validateVerdict(verdict), []);
});

// --- stale verdicts ---

test("isStaleVerdict compares the verdict head against the PR's current head", () => {
  const verdict = baseVerdict();
  assert.equal(isStaleVerdict(verdict, verdict.head), false);
  assert.equal(isStaleVerdict(verdict, "0000000000000000000000000000000000000"), true);
});

test("parseVerdict CLI flags a stale verdict with exit code 4", () => {
  const verdict = baseVerdict();
  const result = Bun.spawnSync(["bun", scriptPath, "--current-head", "0000000000000000000000000000000000000"], {
    stdin: Buffer.from(JSON.stringify(verdict)),
  });
  assert.equal(result.exitCode, 4);
});

// --- malformed verdicts ---

test("parseVerdict rejects malformed JSON", () => {
  assert.throws(() => parseVerdict("{not json"), /not valid JSON/);
});

test("validateVerdict rejects a non-object", () => {
  assert.deepEqual(validateVerdict(null), ["verdict must be an object"]);
  assert.deepEqual(validateVerdict("MERGE_READY"), ["verdict must be an object"]);
});

test("validateVerdict rejects the wrong schema version", () => {
  const errors = validateVerdict(baseVerdict({ schemaVersion: 99 }));
  assert.ok(errors.some((e) => e.includes("schemaVersion")));
});

test("validateVerdict rejects a non-SHA head", () => {
  const errors = validateVerdict(baseVerdict({ head: "not-a-sha" }));
  assert.ok(errors.some((e) => e.includes("head must be")));
});

test("validateVerdict rejects duplicate and overlapping finding IDs", () => {
  const dup = validateVerdict(baseVerdict({ blocking: ["F1", "F1"], status: "NEEDS_WORK", fixScope: "x" }));
  assert.ok(dup.some((e) => e.includes("blocking IDs must be unique")));
  const overlap = validateVerdict(
    baseVerdict({ blocking: ["F1"], nonBlocking: ["F1"], status: "NEEDS_WORK", fixScope: "x" }),
  );
  assert.ok(overlap.some((e) => e.includes("cannot be both blocking and non-blocking")));
});

// --- status-specific rejections ---

test("mechanical-delta-ack is rejected: only full-review is valid this release", () => {
  const errors = validateVerdict(baseVerdict({ sync: "mechanical-delta-ack" }));
  assert.ok(errors.some((e) => e.includes("sync must be one of full-review")));
});

test("NEEDS_WORK without fixScope or blocking findings is rejected", () => {
  const noFixScope = validateVerdict(baseVerdict({ status: "NEEDS_WORK", blocking: ["F1"] }));
  assert.ok(noFixScope.some((e) => e.includes("fixScope is required")));
  const noBlocking = validateVerdict(baseVerdict({ status: "NEEDS_WORK", blocking: [], fixScope: "x" }));
  assert.ok(noBlocking.some((e) => e.includes("at least one blocking finding")));
});

test("BLOCKED without blockedReason is rejected", () => {
  const errors = validateVerdict(baseVerdict({ status: "BLOCKED" }));
  assert.ok(errors.some((e) => e.includes("blockedReason is required")));
});

test("MERGE_READY with blocking findings is rejected", () => {
  const errors = validateVerdict(baseVerdict({ status: "MERGE_READY", blocking: ["F1"] }));
  assert.ok(errors.some((e) => e.includes("MERGE_READY must have zero blocking")));
});

// --- F7: gate/verdict consistency (Codex's cited production probe) --------

test("validateGateEvidence rejects an empty or malformed gates object", () => {
  assert.deepEqual(validateGateEvidence(null), ["gates must be an object"]);
  const errors = validateGateEvidence({});
  assert.ok(errors.some((e) => e.includes("schemaVersion")));
  assert.ok(errors.some((e) => e.includes("headSha")));
  assert.ok(errors.some((e) => e.includes("overall")));
  assert.ok(errors.some((e) => e.includes("checks must be an array")));
  assert.ok(errors.some((e) => e.includes("blockingThreadIds must be an array")));
});

test("validateGateEvidence accepts a well-formed gate-evidence object", () => {
  assert.deepEqual(validateGateEvidence(baseGateEvidence("abc1234")), []);
});

test("validateVerdict rejects a verdict whose gates object is empty or malformed", () => {
  const errors = validateVerdict(baseVerdict({ gates: {} }));
  assert.ok(
    errors.some((e) => e.startsWith("gates.")),
    "gates sub-errors must be prefixed for clarity",
  );
});

test("validateVerdict rejects gates.headSha that does not match the verdict's head", () => {
  const verdict = baseVerdict();
  verdict.gates = baseGateEvidence("0000000000000000000000000000000000000");
  const errors = validateVerdict(verdict);
  assert.ok(errors.some((e) => e.includes("gates.headSha must equal the verdict's head")));
});

test("Codex production probe: MERGE_READY is rejected when gates.overall is fail", () => {
  const verdict = baseVerdict();
  verdict.gates.overall = "fail";
  const errors = validateVerdict(verdict);
  assert.ok(errors.some((e) => e.includes("MERGE_READY requires gates.overall to be pass")));
});

test("Codex production probe: MERGE_READY is rejected when gates carries unresolved blocking threads", () => {
  const verdict = baseVerdict();
  verdict.gates.blockingThreadIds = ["thread-1"];
  const errors = validateVerdict(verdict);
  assert.ok(errors.some((e) => e.includes("MERGE_READY requires zero unresolved blocking review threads")));
});

test("MERGE_READY with a consistent, passing, unblocked gates object has no errors", () => {
  assert.deepEqual(validateVerdict(baseVerdict()), []);
});

const scriptPath = fileURLToPath(new URL("./verdict.mjs", import.meta.url));

test("verdict CLI accepts a valid verdict on stdin and reports stale=false", () => {
  const verdict = baseVerdict();
  const result = Bun.spawnSync(["bun", scriptPath, "--current-head", verdict.head], {
    stdin: Buffer.from(JSON.stringify(verdict)),
  });
  assert.equal(result.exitCode, 0);
});

test("verdict CLI exits 1 on a malformed verdict", () => {
  const result = Bun.spawnSync(["bun", scriptPath], { stdin: Buffer.from("{not json") });
  assert.equal(result.exitCode, 1);
});
