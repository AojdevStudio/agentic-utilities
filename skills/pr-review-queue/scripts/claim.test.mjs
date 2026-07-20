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
const AUTHORIZED = ["worker-a", "worker-b", "worker-c"];

function comment(body, { createdAt, databaseId, worker = "worker-a" }) {
  return { databaseId, createdAt, author: { login: worker }, body };
}

function claim(claimId, opts) {
  const { head = HEAD } = opts;
  return comment(`<!-- pr-review-queue claim id=${claimId} head=${head} -->`, opts);
}

function terminal(type, claimId, opts) {
  const { head = HEAD, reason } = opts;
  const reasonPart = reason ? ` reason=${reason}` : "";
  return comment(`<!-- pr-review-queue ${type} id=${claimId} head=${head}${reasonPart} -->`, opts);
}

function elect(events, overrides = {}) {
  return electClaims(events, {
    head: HEAD,
    now: NOW,
    staleMs: STALE_MS,
    authorizedIdentities: AUTHORIZED,
    ...overrides,
  });
}

// --- exact-body marker recognition (F1: no forgery via prose/quoting) ----

test("parseClaimEvent ignores unrelated comments", () => {
  assert.equal(parseClaimEvent({ body: "just a normal review comment" }), null);
  assert.equal(parseClaimEvent({ body: "" }), null);
});

test("parseClaimEvent requires the ENTIRE body to be exactly the marker, not embedded in prose", () => {
  const marker = "<!-- pr-review-queue claim id=c1 head=abc123 -->";
  assert.equal(
    parseClaimEvent({ databaseId: 1, createdAt: "2026-07-20T00:00:00Z", body: `Sure, I'll claim it: ${marker}` }),
    null,
    "a marker quoted inside a larger comment must not be recognized",
  );
  assert.equal(
    parseClaimEvent({ databaseId: 1, createdAt: "2026-07-20T00:00:00Z", body: `${marker}\nplus extra text` }),
    null,
    "trailing prose after the marker must not be recognized",
  );
  assert.equal(
    parseClaimEvent({ databaseId: 1, createdAt: "2026-07-20T00:00:00Z", body: `> ${marker}` }),
    null,
    "a quoted (blockquoted) marker must not be recognized",
  );
});

test("parseClaimEvent recognizes the marker alone, tolerating only whitespace normalization", () => {
  const event = parseClaimEvent({
    databaseId: 5,
    createdAt: "2026-07-20T00:00:00Z",
    author: { login: "worker-a" },
    body: "  <!-- pr-review-queue claim id=c1 head=abc123 -->  \r\n",
  });
  assert.equal(event.type, "claim");
  assert.equal(event.claimId, "c1");
});

test("parseClaimEvent requires an immutable databaseId", () => {
  assert.throws(
    () =>
      parseClaimEvent({ createdAt: "2026-07-20T00:00:00Z", body: "<!-- pr-review-queue claim id=c1 head=abc123 -->" }),
    /missing immutable databaseId/,
  );
});

// --- authorization: unauthorized claim events never win --------------------

test("a claim from an unauthorized author is never recognized as a live claim", () => {
  const events = [
    parseClaimEvent(claim("c-forged", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "random-passerby" })),
  ];
  const election = elect(events);
  assert.equal(election.claims.length, 0, "an unauthorized claimant must produce zero recognized claims");
  assert.equal(election.winner, null);
});

// --- authorization: terminal events must come from the right identity ------

test("F1: a hostile complete event from a different author cannot suppress the real owner's review", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("complete", "c1", { createdAt: "2026-07-20T00:01:00Z", databaseId: 2, worker: "worker-b" }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c1");
  assert.equal(election.winner.state, "active", "a complete event from a non-owner must be ignored entirely");
});

test("F1: a hostile release event from a different author cannot force a reclaim", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("release", "c1", { createdAt: "2026-07-20T00:01:00Z", databaseId: 2, worker: "worker-c" }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.winner.state, "active");
});

test("F1: an abandon event from an unauthorized identity is ignored, even quoting the exact head", () => {
  const staleCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c-dead", { createdAt: staleCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("abandon", "c-dead", {
        createdAt: new Date(NOW - 500).toISOString(),
        databaseId: 2,
        worker: "random-passerby",
        reason: "stale",
      }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c-dead");
  assert.equal(election.winner.state, "active", "abandon from an unauthorized identity must not apply");
});

test("F1: an abandon event from an authorized identity other than the owner is honored (this is the reclaim path)", () => {
  const staleCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c-dead", { createdAt: staleCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("abandon", "c-dead", {
        createdAt: new Date(NOW - 500).toISOString(),
        databaseId: 2,
        worker: "worker-b",
        reason: "stale",
      }),
    ),
  ];
  const election = elect(events);
  const dead = election.claims.find((c) => c.claimId === "c-dead");
  assert.equal(dead.state, "abandoned");
});

// --- terminal binding: wrong head, before the claim -------------------------

test("F1: a terminal event naming a different head does not bind to the claim", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("complete", "c1", {
        createdAt: "2026-07-20T00:01:00Z",
        databaseId: 2,
        worker: "worker-a",
        head: "wrong-head",
      }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.winner.state, "active", "a complete event for a different head must not apply");
});

test("F1: a terminal event predating its own claim in total order does not bind", () => {
  const events = [
    // The "complete" event is posted (lower databaseId / earlier time) BEFORE the claim it claims to terminate.
    parseClaimEvent(
      terminal("complete", "c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" }),
    ),
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:01:00Z", databaseId: 2, worker: "worker-a" })),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c1");
  assert.equal(election.winner.state, "active", "a terminal event that sorts before its own claim must be ignored");
});

// --- concurrent-claim / tied-timestamp / identical-label (unchanged from prior rounds) ---

test("concurrent claims elect the earlier createdAt as winner", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:10:00Z", databaseId: 10, worker: "worker-a" })),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:10:05Z", databaseId: 11, worker: "worker-b" })),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c1");
});

test("tied timestamps are broken by the immutable databaseId, not arrival order", () => {
  const sameInstant = "2026-07-20T00:10:00Z";
  const events = [
    parseClaimEvent(claim("c-later-db", { createdAt: sameInstant, databaseId: 99, worker: "worker-a" })),
    parseClaimEvent(claim("c-earlier-db", { createdAt: sameInstant, databaseId: 12, worker: "worker-b" })),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c-earlier-db");
});

test("claims are distinguished by claim_id, not by the self-declared worker label", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:00:01Z", databaseId: 2, worker: "worker-a" })),
  ];
  const election = elect(events);
  assert.equal(election.claims.length, 2);
  assert.equal(election.winner.claimId, "c1");
});

// --- F4: renewable lease, not fixed elapsed-time-from-creation --------------

test("F4: a claim renewed by its owner stays non-reclaimable past the original creation window", () => {
  const oldCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString(); // would be stale by creation time alone
  const recentRenewal = new Date(NOW - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c-alive", { createdAt: oldCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(terminal("renew", "c-alive", { createdAt: recentRenewal, databaseId: 2, worker: "worker-a" })),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c-alive");
  assert.equal(election.winner.reclaimable, false, "a renewed lease must not be reclaimable");
  assert.equal(election.winner.lastActivityAt, recentRenewal);
});

test("F4: a renew event from a different author is ignored", () => {
  const oldCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c1", { createdAt: oldCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("renew", "c1", { createdAt: new Date(NOW - 1000).toISOString(), databaseId: 2, worker: "worker-b" }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.winner.reclaimable, true, "a forged renewal from a non-owner must not extend the lease");
});

test("F4: a renew posted after abandonment does not resurrect the claim", () => {
  const oldCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c1", { createdAt: oldCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("abandon", "c1", {
        createdAt: new Date(NOW - 2000).toISOString(),
        databaseId: 2,
        worker: "worker-b",
        reason: "stale",
      }),
    ),
    parseClaimEvent(
      terminal("renew", "c1", { createdAt: new Date(NOW - 1000).toISOString(), databaseId: 3, worker: "worker-a" }),
    ),
  ];
  const election = elect(events);
  const claimState = election.claims.find((c) => c.claimId === "c1");
  assert.equal(claimState.state, "abandoned", "a renewal cannot resurrect an already-abandoned claim");
});

// --- reclamation and completed-wins (unchanged behavior, re-verified under new auth model) ---

test("reclamation: an abandon event followed by a new claim replaces the stale winner", () => {
  const staleCreatedAt = new Date(NOW - STALE_MS - 1000).toISOString();
  const events = [
    parseClaimEvent(claim("c-dead", { createdAt: staleCreatedAt, databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("abandon", "c-dead", {
        createdAt: new Date(NOW - 500).toISOString(),
        databaseId: 2,
        worker: "worker-b",
        reason: "stale",
      }),
    ),
    parseClaimEvent(
      claim("c-new", { createdAt: new Date(NOW - 400).toISOString(), databaseId: 3, worker: "worker-b" }),
    ),
  ];
  const election = elect(events);
  const dead = election.claims.find((c) => c.claimId === "c-dead");
  assert.equal(dead.state, "abandoned");
  assert.equal(election.winner.claimId, "c-new");
});

test("a completed claim wins election over a later claim for the same head", () => {
  const events = [
    parseClaimEvent(claim("c1", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })),
    parseClaimEvent(
      terminal("complete", "c1", { createdAt: "2026-07-20T00:05:00Z", databaseId: 2, worker: "worker-a" }),
    ),
    parseClaimEvent(claim("c2", { createdAt: "2026-07-20T00:06:00Z", databaseId: 3, worker: "worker-b" })),
  ];
  const election = elect(events);
  assert.equal(election.winner.claimId, "c1");
  assert.equal(election.winner.state, "completed");
  assert.equal(needsGateOnlyReevaluation(election), true);
  assert.equal(needsFullReview(election), false);
});

test("terminal events referencing an unknown claim_id are ignored", () => {
  const events = [
    parseClaimEvent(
      terminal("complete", "ghost", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" }),
    ),
  ];
  const election = elect(events);
  assert.equal(election.claims.length, 0);
  assert.equal(election.winner, null);
});

// --- F1 + task 38: three-worker winner / loser / reclaimer end-to-end ------

test("three-worker lifecycle: winner, a loser who self-releases, and a later reclaimer", () => {
  const events = [];
  // worker-a claims first: wins the race.
  events.push(parseClaimEvent(claim("c-a", { createdAt: "2026-07-20T00:00:00Z", databaseId: 1, worker: "worker-a" })));
  // worker-b claims a beat later: loses the election.
  events.push(parseClaimEvent(claim("c-b", { createdAt: "2026-07-20T00:00:01Z", databaseId: 2, worker: "worker-b" })));

  let election = elect(events);
  assert.equal(election.winner.claimId, "c-a", "worker-a's earlier claim must win");
  const loser = election.claims.find((c) => c.claimId === "c-b");
  assert.equal(loser.state, "active", "worker-b's claim exists but is not the winner");

  // worker-b, seeing it lost, self-releases so it can never later become winner.
  events.push(
    parseClaimEvent(
      terminal("release", "c-b", { createdAt: "2026-07-20T00:00:02Z", databaseId: 3, worker: "worker-b" }),
    ),
  );
  election = elect(events);
  const releasedLoser = election.claims.find((c) => c.claimId === "c-b");
  assert.equal(releasedLoser.state, "released");
  assert.equal(election.winner.claimId, "c-a", "the winner is unaffected by the loser's release");

  // Time passes; worker-a's claim goes stale with no renewal.
  const laterNow = NOW;
  election = electClaims(events, { head: HEAD, now: laterNow, staleMs: STALE_MS, authorizedIdentities: AUTHORIZED });
  assert.equal(election.winner.reclaimable, true, "worker-a's claim must be reclaimable once stale");

  // worker-c reclaims: abandons worker-a's stale claim, then claims fresh.
  events.push(
    parseClaimEvent(
      terminal("abandon", "c-a", {
        createdAt: new Date(laterNow - 500).toISOString(),
        databaseId: 4,
        worker: "worker-c",
        reason: "stale",
      }),
    ),
  );
  events.push(
    parseClaimEvent(
      claim("c-c", { createdAt: new Date(laterNow - 400).toISOString(), databaseId: 5, worker: "worker-c" }),
    ),
  );
  election = electClaims(events, { head: HEAD, now: laterNow, staleMs: STALE_MS, authorizedIdentities: AUTHORIZED });

  const original = election.claims.find((c) => c.claimId === "c-a");
  assert.equal(original.state, "abandoned");
  assert.equal(election.winner.claimId, "c-c", "worker-c's fresh claim must now be the winner");

  // The released loser can never resurrect and win, even after all this.
  assert.notEqual(election.winner.claimId, "c-b");
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
    "--authorized",
    "fleet-bot",
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
    "--authorized",
    "fleet-bot",
  ]);
  assert.equal(result.exitCode, 0);
  const election = await new Response(result.stdout).json();
  assert.equal(election.winner.claimId, "c1");
});

test("claim CLI requires --authorized", () => {
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
  assert.equal(result.exitCode, 1);
});
