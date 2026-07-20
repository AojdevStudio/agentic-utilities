import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  collectClaimEvents,
  electClaims,
  needsFullReview,
  needsGateOnlyReevaluation,
  parseClaimEvent,
} from "./claim.mjs";

const HEAD = "abc123";
const NOW = Date.parse("2026-07-20T00:30:00Z");
const STALE_MS = 20 * 60 * 1000;

function claim(claimId, { createdAt, databaseId, worker = "worker-1", head = HEAD }) {
  return {
    databaseId,
    createdAt,
    author: { login: worker },
    body: `<!-- pr-review-queue claim id=${claimId} head=${head} -->`,
  };
}

function terminal(type, claimId, { createdAt, databaseId, reason }) {
  const reasonPart = reason ? ` reason=${reason}` : "";
  return {
    databaseId,
    createdAt,
    author: { login: "worker-x" },
    body: `<!-- pr-review-queue ${type} id=${claimId} head=${HEAD}${reasonPart} -->`,
  };
}

test("parseClaimEvent ignores unrelated comments", () => {
  assert.equal(parseClaimEvent({ body: "just a normal review comment" }), null);
  assert.equal(parseClaimEvent({ body: "" }), null);
});

test("parseClaimEvent extracts type, claimId, head, worker, and immutable ordering keys", () => {
  const event = parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 5 }));
  assert.deepEqual(event, {
    type: "claim",
    claimId: "c1",
    head: HEAD,
    reason: null,
    worker: "worker-1",
    databaseId: 5,
    createdAt: "2026-07-20T00:00:00Z",
  });
});

test("parseClaimEvent requires an immutable databaseId", () => {
  assert.throws(
    () =>
      parseClaimEvent({ createdAt: "2026-07-20T00:00:00Z", body: "<!-- pr-review-queue claim id=c1 head=abc123 -->" }),
    /missing immutable databaseId/,
  );
});

// --- concurrent-claim: two workers claim the same head; total order decides ---
test("concurrent claims elect the earlier createdAt as winner", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:10:00Z", databaseId: 10, worker: "worker-a" })),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:10:05Z", databaseId: 11, worker: "worker-b" })),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.winner.claimId, "c1");
  assert.equal(election.claims.length, 2);
});

// --- tied-timestamp: identical createdAt, tiebreak by immutable databaseId ---
test("tied timestamps are broken by the immutable databaseId, not arrival order", () => {
  const sameInstant = "2026-07-20T00:10:00Z";
  const events = [
    parseClaimEvent(claim("c-later-db", { createdAt: sameInstant, databaseId: 99, worker: "worker-a" })),
    parseClaimEvent(claim("c-earlier-db", { createdAt: sameInstant, databaseId: 12, worker: "worker-b" })),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.winner.claimId, "c-earlier-db", "the lower databaseId must win a timestamp tie");
});

// --- identical-label: two distinct claim_ids from the same self-declared worker label ---
test("claims are distinguished by claim_id, not by the self-declared worker label", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "reviewer-1" })),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:00:01Z", databaseId: 2, worker: "reviewer-1" })),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.claims.length, 2, "identical labels must not collapse distinct claim_ids");
  assert.equal(election.winner.claimId, "c1");
});

// --- worker-death: a claim with no terminal event becomes reclaimable after staleMs ---
test("a claim with no terminal event is reclaimable once it exceeds the staleness window", () => {
  const oldEnough = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [parseClaimEvent(claim("c-dead", { createdAt: oldEnough, databaseId: 1, worker: "worker-a" }))];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.winner.claimId, "c-dead");
  assert.equal(election.winner.reclaimable, true);
});

test("a fresh claim with no terminal event is active but not reclaimable", () => {
  const events = [parseClaimEvent(claim("c-fresh", { createdAt: new Date(NOW - 1000).toISOString(), databaseId: 1 }))];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.winner.state, "active");
  assert.equal(election.winner.reclaimable, false);
});

// --- reclamation: abandon the stale claim, then a fresh claim becomes the new winner ---
test("reclamation: an abandon event followed by a new claim replaces the stale winner", () => {
  const staleCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c-dead", { createdAt: staleCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("abandon", "c-dead", { createdAt: new Date(NOW - 500).toISOString(), databaseId: 2, reason: "stale" }),
    ),
    parseClaimEvent(
      claim("c-new", { createdAt: new Date(NOW - 400).toISOString(), databaseId: 3, worker: "worker-b" }),
    ),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  const dead = election.claims.find((c) => c.claimId === "c-dead");
  assert.equal(dead.state, "abandoned");
  assert.equal(election.winner.claimId, "c-new");
  assert.equal(election.winner.state, "active");
});

test("a completed claim wins election over a later claim for the same head", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1 })),
    parseClaimEvent(terminal("complete", "c1", { createdAt: "2026-07-20T00:05:00Z", databaseId: 2 })),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:06:00Z", databaseId: 3, worker: "worker-b" })),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.winner.claimId, "c1");
  assert.equal(election.winner.state, "completed");
  assert.equal(needsGateOnlyReevaluation(election), true);
  assert.equal(needsFullReview(election), false);
});

test("released and abandoned claims never win, and a duplicate claim event is idempotent", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1 })),
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1 })), // duplicate delivery
    parseClaimEvent(terminal("release", "c1", { createdAt: "2026-07-20T00:01:00Z", databaseId: 2 })),
  ];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(
    election.claims.length,
    1,
    "a duplicate claim delivery for the same claim_id must not create two entries",
  );
  assert.equal(election.claims[0].state, "released");
  assert.equal(election.winner, null);
  assert.equal(needsFullReview(election), true);
});

test("terminal events referencing an unknown claim_id are ignored", () => {
  const events = [parseClaimEvent(terminal("complete", "ghost", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1 }))];
  const election = electClaims(events, { head: HEAD, now: NOW, staleMs: STALE_MS });
  assert.equal(election.claims.length, 0);
  assert.equal(election.winner, null);
});

// --- pagination ---
const fixture = await Bun.file(new URL("../fixtures/claim-pages.json", import.meta.url)).json();

test("claim collection follows every page and stops at hasNextPage=false", async () => {
  const cursors = [];
  const result = await collectClaimEvents(async (cursor) => {
    cursors.push(cursor ?? null);
    return fixture.pages[cursors.length - 1];
  });
  assert.deepEqual(cursors, [null, "cursor-1"]);
  assert.equal(result.headSha, "abc123");
  assert.equal(result.events.length, 3);
});

test("claim collection rejects head drift between pages", async () => {
  const pages = structuredClone(fixture.pages);
  pages[1].data.repository.pullRequest.headRefOid = "changed-head";
  let pageIndex = 0;
  await assert.rejects(
    collectClaimEvents(async () => pages[pageIndex++]),
    /head changed during pagination/,
  );
});

const scriptPath = fileURLToPath(new URL("./claim.mjs", import.meta.url));
const fixturePath = fileURLToPath(new URL("../fixtures/claim-pages.json", import.meta.url));

test("claim CLI rejects a stale expected head", () => {
  const result = Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "fixture/repository",
    "--pr",
    "1",
    "--expected-head",
    "stale-head",
    "--fixture",
    fixturePath,
  ]);
  assert.equal(result.exitCode, 1);
});

test("claim CLI elects the winner across paginated fixture pages", async () => {
  const result = Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "fixture/repository",
    "--pr",
    "1",
    "--expected-head",
    "abc123",
    "--fixture",
    fixturePath,
  ]);
  assert.equal(result.exitCode, 0);
  const election = await new Response(result.stdout).json();
  assert.equal(election.winner.claimId, "c1");
});
