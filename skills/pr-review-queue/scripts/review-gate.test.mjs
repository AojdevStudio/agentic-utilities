import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  assertHeadUnchanged,
  classifyCheckNode,
  collectReviewThreads,
  gateEvidence,
  isAutomatedAuthor,
  parseCheckPayload,
  reviewThreadEvidence,
  summarizeGates,
} from "./review-gate.mjs";

const fixture = await Bun.file(new URL("../fixtures/gate-evidence.json", import.meta.url)).json();

test("isAutomatedAuthor classifies bot typename and known automation logins", () => {
  assert.equal(isAutomatedAuthor({ login: "coderabbitai", __typename: "Bot" }), true);
  assert.equal(isAutomatedAuthor({ login: "dependabot[bot]", __typename: "User" }), true);
  assert.equal(isAutomatedAuthor({ login: "a-human", __typename: "User" }), false);
  assert.equal(isAutomatedAuthor(null), false);
});

test("review thread collection follows every page and stays pinned to one head", async () => {
  let pageIndex = 0;
  const result = await collectReviewThreads(async () => fixture.threadPages[pageIndex++]);
  assert.equal(result.headSha, "abc123");
  assert.equal(result.threads.length, 4);
});

test("review thread collection rejects head drift between pages", async () => {
  const pages = structuredClone(fixture.threadPages);
  pages[1].data.repository.pullRequest.headRefOid = "changed-head";
  let pageIndex = 0;
  await assert.rejects(
    collectReviewThreads(async () => pages[pageIndex++]),
    /head changed during pagination/,
  );
});

test("reviewThreadEvidence separates human-blocking from automated-blocking", async () => {
  let pageIndex = 0;
  const result = await collectReviewThreads(async () => fixture.threadPages[pageIndex++]);
  const evidence = reviewThreadEvidence(result);
  assert.deepEqual(evidence.blockingThreadIds, ["thread-human-unresolved", "thread-automated-unresolved"]);
  const automated = evidence.threads.find((t) => t.id === "thread-automated-unresolved");
  assert.equal(automated.automated, true);
  const human = evidence.threads.find((t) => t.id === "thread-human-unresolved");
  assert.equal(human.automated, false);
});

test("classifyCheckNode maps CheckRun and StatusContext to the six-state model", () => {
  assert.deepEqual(
    classifyCheckNode({ __typename: "CheckRun", name: "lint", status: "COMPLETED", conclusion: "SUCCESS" }),
    {
      name: "lint",
      state: "pass",
    },
  );
  assert.deepEqual(
    classifyCheckNode({ __typename: "CheckRun", name: "test", status: "COMPLETED", conclusion: "FAILURE" }),
    {
      name: "test",
      state: "fail",
    },
  );
  assert.deepEqual(
    classifyCheckNode({ __typename: "CheckRun", name: "build", status: "COMPLETED", conclusion: "CANCELLED" }),
    {
      name: "build",
      state: "cancelled",
    },
  );
  assert.deepEqual(
    classifyCheckNode({ __typename: "CheckRun", name: "e2e", status: "IN_PROGRESS", conclusion: null }),
    {
      name: "e2e",
      state: "pending",
    },
  );
  assert.deepEqual(classifyCheckNode({ __typename: "StatusContext", context: "legacy/ci", state: "ERROR" }), {
    name: "legacy/ci",
    state: "error",
  });
  assert.throws(() => classifyCheckNode({ __typename: "Something" }), /unknown check node type/);
});

test("summarizeGates: a failing required check dominates a passing advisory one", () => {
  const checks = [
    { name: "lint", state: "pass" },
    { name: "test", state: "fail" },
    { name: "advisory/coverage", state: "pending" },
  ];
  const summary = summarizeGates(checks, ["lint", "test"]);
  assert.equal(summary.overall, "fail");
  assert.equal(summary.checks.find((c) => c.name === "advisory/coverage").required, false);
});

test("summarizeGates: no required checks observed is unavailable, not a silent pass", () => {
  const summary = summarizeGates([{ name: "advisory-only", state: "pass" }], ["required-but-missing"]);
  assert.equal(summary.overall, "unavailable");
});

test("summarizeGates: error outranks fail, which outranks cancelled, which outranks pending", () => {
  assert.equal(
    summarizeGates(
      [
        { name: "a", state: "error" },
        { name: "b", state: "fail" },
      ],
      ["a", "b"],
    ).overall,
    "error",
  );
  assert.equal(
    summarizeGates(
      [
        { name: "a", state: "fail" },
        { name: "b", state: "cancelled" },
      ],
      ["a", "b"],
    ).overall,
    "fail",
  );
  assert.equal(
    summarizeGates(
      [
        { name: "a", state: "cancelled" },
        { name: "b", state: "pending" },
      ],
      ["a", "b"],
    ).overall,
    "cancelled",
  );
});

test("parseCheckPayload extracts head, checks, and required names from the combined query", () => {
  const result = parseCheckPayload(fixture.checks);
  assert.equal(result.headSha, "abc123");
  assert.equal(result.checks.length, 3);
  assert.deepEqual(result.requiredNames, ["lint", "test"]);
});

test("gateEvidence rejects combining thread and check results read for different heads", async () => {
  let pageIndex = 0;
  const threadResult = await collectReviewThreads(async () => fixture.threadPages[pageIndex++]);
  const checkResult = { ...parseCheckPayload(fixture.checks), headSha: "different-head" };
  assert.throws(() => gateEvidence(threadResult, checkResult, "2026-07-20T00:00:00Z"), /different heads/);
});

test("gateEvidence ties threads, gates, and a timestamp to one head", async () => {
  let pageIndex = 0;
  const threadResult = await collectReviewThreads(async () => fixture.threadPages[pageIndex++]);
  const checkResult = parseCheckPayload(fixture.checks);
  const evidence = gateEvidence(threadResult, checkResult, "2026-07-20T00:00:00Z");
  assert.equal(evidence.headSha, "abc123");
  assert.equal(evidence.checkedAt, "2026-07-20T00:00:00Z");
  assert.deepEqual(evidence.humanBlockingThreadIds, ["thread-human-unresolved"]);
  assert.deepEqual(evidence.automatedBlockingThreadIds, ["thread-automated-unresolved"]);
  assert.equal(evidence.gates.overall, "fail");
});

test("assertHeadUnchanged throws when the reviewed head no longer matches", () => {
  const evidence = { headSha: "abc123" };
  assert.equal(assertHeadUnchanged(evidence, "abc123"), evidence);
  assert.throws(() => assertHeadUnchanged(evidence, "def456"), /head changed since gate evidence/);
});

const scriptPath = fileURLToPath(new URL("./review-gate.mjs", import.meta.url));
const fixturePath = fileURLToPath(new URL("../fixtures/gate-evidence.json", import.meta.url));

function runFixtureGate(expectedHead) {
  return Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "fixture/repository",
    "--pr",
    "1",
    "--expected-head",
    expectedHead,
    "--fixture",
    fixturePath,
  ]);
}

test("review-gate CLI exits 3 when required checks fail even with zero blocking threads", () => {
  const result = runFixtureGate("abc123");
  assert.equal(result.exitCode, 3);
});

test("review-gate CLI rejects a stale expected head", () => {
  const result = runFixtureGate("stale-head");
  assert.equal(result.exitCode, 1);
});
