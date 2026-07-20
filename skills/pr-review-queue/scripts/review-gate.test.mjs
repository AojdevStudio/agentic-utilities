import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  assertHeadUnchanged,
  classifyCheckNode,
  collectCheckContexts,
  collectReviewThreads,
  gateEvidence,
  isAutomatedAuthor,
  parseRequiredNames,
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

// --- F7: paginate check contexts to exhaustion ------------------------------

test("collectCheckContexts follows every page and stays pinned to one head", async () => {
  let pageIndex = 0;
  const result = await collectCheckContexts(async () => fixture.checkPages[pageIndex++]);
  assert.equal(result.headSha, "abc123");
  assert.equal(
    result.checks.length,
    3,
    "must not stop at page one; the advisory check on page two must be collected too",
  );
});

test("collectCheckContexts rejects a repeated cursor instead of looping forever", async () => {
  const pages = structuredClone(fixture.checkPages);
  const rollup = (page) => page.data.repository.pullRequest.commits.nodes[0].commit.statusCheckRollup;
  rollup(pages[1]).contexts.pageInfo.hasNextPage = true;
  rollup(pages[1]).contexts.pageInfo.endCursor = "check-cursor-1"; // same cursor page one already handed out
  let pageIndex = 0;
  await assert.rejects(
    collectCheckContexts(async () => pages[pageIndex++]),
    /invalid check context pagination cursor/,
  );
});

test("collectCheckContexts rejects head drift between check-context pages", async () => {
  const pages = structuredClone(fixture.checkPages);
  pages[1].data.repository.pullRequest.headRefOid = "changed-head";
  let pageIndex = 0;
  await assert.rejects(
    collectCheckContexts(async () => pages[pageIndex++]),
    /head changed during pagination/,
  );
});

test("parseRequiredNames extracts the required check names", () => {
  assert.deepEqual(parseRequiredNames(fixture.requiredNames), ["lint", "test", "e2e"]);
});

// --- F7: a required check with no matching observed context is synthesized
// as unavailable, not silently dropped -------------------------------------

test("summarizeGates synthesizes a missing required check as unavailable", () => {
  const summary = summarizeGates([{ name: "lint", state: "pass" }], ["lint", "never-reported"]);
  const missing = summary.checks.find((c) => c.name === "never-reported");
  assert.ok(missing, "the missing required check must still appear in the summary");
  assert.equal(missing.state, "unavailable");
  assert.equal(missing.required, true);
  assert.equal(missing.synthesized, true);
});

test("summarizeGates: a synthesized missing required check drives overall unavailable/fail correctly", () => {
  // All observed required checks pass, but one required name never reported at all.
  const allPass = summarizeGates([{ name: "lint", state: "pass" }], ["lint", "ghost-check"]);
  assert.equal(allPass.overall, "unavailable", "a missing required check must not be blessed as passing");

  // A failing observed check still outranks a synthesized-unavailable one.
  const oneFails = summarizeGates(
    [
      { name: "lint", state: "pass" },
      { name: "test", state: "fail" },
    ],
    ["lint", "test", "ghost-check"],
  );
  assert.equal(oneFails.overall, "fail");
});

test("summarizeGates: an observed subset can never be blessed as passing when a named required check is absent", () => {
  // This is the exact adversarial shape the audit named: only a subset of
  // required checks was observed, all of which pass; the summary must not
  // treat the PR as ready.
  const summary = summarizeGates(
    [{ name: "lint", state: "pass" }],
    ["lint", "test", "e2e"], // test and e2e never reported
  );
  assert.notEqual(summary.overall, "pass");
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

// --- gateEvidence: flat shape, head consistency, timestamp ------------------

async function collectFixtureEvidence(now) {
  let threadPageIndex = 0;
  const threadResult = await collectReviewThreads(async () => fixture.threadPages[threadPageIndex++]);
  let checkPageIndex = 0;
  const checkResult = await collectCheckContexts(async () => fixture.checkPages[checkPageIndex++]);
  const requiredNames = parseRequiredNames(fixture.requiredNames);
  return gateEvidence(threadResult, { ...checkResult, requiredNames }, now);
}

test("gateEvidence rejects combining thread and check results read for different heads", async () => {
  let pageIndex = 0;
  const threadResult = await collectReviewThreads(async () => fixture.threadPages[pageIndex++]);
  const checkResult = { headSha: "different-head", checks: [], requiredNames: [] };
  assert.throws(() => gateEvidence(threadResult, checkResult, "2026-07-20T00:00:00Z"), /different heads/);
});

test("gateEvidence ties threads, gates, and a timestamp to one head, in a flat schema", async () => {
  const evidence = await collectFixtureEvidence("2026-07-20T00:00:00Z");
  assert.equal(evidence.schemaVersion, 1);
  assert.equal(evidence.headSha, "abc123");
  assert.equal(evidence.checkedAt, "2026-07-20T00:00:00Z");
  assert.deepEqual(evidence.humanBlockingThreadIds, ["thread-human-unresolved"]);
  assert.deepEqual(evidence.automatedBlockingThreadIds, ["thread-automated-unresolved"]);
  assert.equal(evidence.overall, "fail", "checks and overall are flat top-level fields, not nested under .gates");
  assert.ok(Array.isArray(evidence.checks));
  assert.ok(evidence.checks.some((c) => c.name === "e2e" && c.synthesized === true));
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
