import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { collectOpenPullRequests, reviewableQueue } from "./queue.mjs";

const fixture = await Bun.file(new URL("../fixtures/queue-pages.json", import.meta.url)).json();

test("collectOpenPullRequests follows every page regardless of count", async () => {
  let pageIndex = 0;
  const pullRequests = await collectOpenPullRequests(async () => fixture.pages[pageIndex++]);
  assert.equal(pullRequests.length, 4, "must not stop at any fixed ceiling, only at hasNextPage=false");
});

test("collectOpenPullRequests rejects a repeated cursor instead of looping forever", async () => {
  const pages = structuredClone(fixture.pages);
  // Force a second page: still has more, but hands back the same cursor it was fetched with.
  pages[1].data.repository.pullRequests.pageInfo.hasNextPage = true;
  pages[1].data.repository.pullRequests.pageInfo.endCursor = "cursor-1";
  let pageIndex = 0;
  await assert.rejects(
    collectOpenPullRequests(async () => pages[pageIndex++]),
    /invalid pull request pagination cursor/,
  );
});

test("reviewableQueue filters drafts and sorts oldest-first", async () => {
  let pageIndex = 0;
  const pullRequests = await collectOpenPullRequests(async () => fixture.pages[pageIndex++]);
  assert.deepEqual(reviewableQueue(pullRequests), [12, 35, 41]);
});

const scriptPath = fileURLToPath(new URL("./queue.mjs", import.meta.url));
const fixturePath = fileURLToPath(new URL("../fixtures/queue-pages.json", import.meta.url));

test("queue CLI reports the full paginated, filtered, sorted queue", async () => {
  const result = Bun.spawnSync(["bun", scriptPath, "--repo", "fixture/repository", "--fixture", fixturePath]);
  assert.equal(result.exitCode, 0);
  const output = await new Response(result.stdout).json();
  assert.equal(output.total, 4);
  assert.deepEqual(output.queue, [12, 35, 41]);
});
