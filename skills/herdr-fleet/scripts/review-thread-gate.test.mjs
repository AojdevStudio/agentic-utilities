import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { collectReviewThreads, reviewThreadEvidence } from "./review-thread-gate.mjs";

const fixture = await Bun.file(new URL("../fixtures/review-thread-pages.json", import.meta.url)).json();

test("review thread collection follows every page", async () => {
  const cursors = [];
  const result = await collectReviewThreads(async (cursor) => {
    cursors.push(cursor ?? null);
    return fixture.pages[cursors.length - 1];
  });
  assert.deepEqual(cursors, [null, "cursor-1"]);
  assert.equal(result.threads.length, 4);
  assert.equal(result.headSha, "abc123");
});

test("review thread collection rejects head drift between pages", async () => {
  const pages = structuredClone(fixture.pages);
  pages[1].data.repository.pullRequest.headRefOid = "changed-head";
  let pageIndex = 0;
  await assert.rejects(
    collectReviewThreads(async () => pages[pageIndex++]),
    /head changed during pagination/,
  );
});

test("review gate records all states and blocks only current unresolved threads", async () => {
  let pageIndex = 0;
  const result = await collectReviewThreads(async () => fixture.pages[pageIndex++]);
  const evidence = reviewThreadEvidence(result);
  assert.deepEqual(
    evidence.threads.map(({ id, url, isResolved, isOutdated }) => ({ id, url, isResolved, isOutdated })),
    [
      {
        id: "thread-unresolved",
        url: "https://example.test/unresolved",
        isResolved: false,
        isOutdated: false,
      },
      {
        id: "thread-resolved",
        url: "https://example.test/resolved",
        isResolved: true,
        isOutdated: false,
      },
      {
        id: "thread-outdated",
        url: "https://example.test/outdated",
        isResolved: false,
        isOutdated: true,
      },
      {
        id: "thread-page-two-unresolved",
        url: "https://example.test/page-two",
        isResolved: false,
        isOutdated: false,
      },
    ],
  );
  assert.deepEqual(evidence.blockingThreadIds, ["thread-unresolved", "thread-page-two-unresolved"]);
});

const scriptPath = fileURLToPath(new URL("./review-thread-gate.mjs", import.meta.url));
const fixturePath = fileURLToPath(new URL("../fixtures/review-thread-pages.json", import.meta.url));

function runFixtureGate(expectedHead) {
  return Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "fixture/repository",
    "--pr",
    "32",
    "--expected-head",
    expectedHead,
    "--fixture",
    fixturePath,
  ]);
}

test("review gate CLI rejects a stale expected head", () => {
  assert.equal(runFixtureGate("stale-head").exitCode, 1);
});

test("review gate CLI exits three for current unresolved threads", async () => {
  const result = runFixtureGate("abc123");
  assert.equal(result.exitCode, 3);
  const evidence = await new Response(result.stdout).json();
  assert.deepEqual(evidence.blockingThreadIds, ["thread-unresolved", "thread-page-two-unresolved"]);
});
