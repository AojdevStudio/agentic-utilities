#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

// Claim lifecycle: election and state folding over an append-only log of
// GitHub PR comments. Every claim-related comment is one event; state is
// always recomputed by folding the full event log, never by editing or
// trusting a single comment's current text. This makes the whole thing
// race-safe: two workers can both read-then-post concurrently, and
// whichever comment GitHub actually assigned the lower (createdAt,
// databaseId) total-order position wins, deterministically, for every
// reader who re-folds the same log.

const MARKER = /<!-- pr-review-queue (claim|release|abandon|complete) id=(\S+) head=(\S+)(?: reason=(\S+))? -->/;

/** Parse a single GitHub comment into a claim event, or null if it isn't one. */
export function parseClaimEvent(comment) {
  const match = typeof comment?.body === "string" ? comment.body.match(MARKER) : null;
  if (!match) return null;
  const [, type, claimId, head, reason] = match;
  if (comment.databaseId === undefined || comment.databaseId === null) {
    throw new Error(`claim event missing immutable databaseId: ${claimId}`);
  }
  if (!comment.createdAt) throw new Error(`claim event missing createdAt: ${claimId}`);
  return {
    type,
    claimId,
    head,
    reason: reason ?? null,
    worker: comment.author?.login ?? null,
    databaseId: comment.databaseId,
    createdAt: comment.createdAt,
  };
}

/** Total order: createdAt first, then the immutable numeric databaseId as a tiebreaker. */
function compareTotalOrder(a, b) {
  const byTime = Date.parse(a.createdAt) - Date.parse(b.createdAt);
  if (byTime !== 0) return byTime;
  return Number(a.databaseId) - Number(b.databaseId);
}

function isBefore(a, b) {
  return compareTotalOrder(a, b) < 0;
}

/**
 * Fold every claim event for one PR into per-claim state and elect the
 * current winner for `head`. Events for other heads are kept (so their
 * claims still show up, e.g. as stale/abandoned history) but never win
 * election for `head`.
 */
export function electClaims(events, { head, now, staleMs }) {
  if (!head) throw new Error("electClaims requires the exact head SHA being evaluated");
  if (!Number.isFinite(now)) throw new Error("electClaims requires a numeric now");
  if (!Number.isFinite(staleMs) || staleMs <= 0) throw new Error("electClaims requires a positive staleMs");

  const firstClaimByClaimId = new Map();
  for (const event of events) {
    if (event.type !== "claim") continue;
    const existing = firstClaimByClaimId.get(event.claimId);
    if (!existing || isBefore(event, existing)) firstClaimByClaimId.set(event.claimId, event);
  }

  const firstTerminalByClaimId = new Map();
  for (const event of events) {
    if (event.type === "claim") continue;
    if (!firstClaimByClaimId.has(event.claimId)) continue; // terminal event with no matching claim: ignore
    const existing = firstTerminalByClaimId.get(event.claimId);
    if (!existing || isBefore(event, existing)) firstTerminalByClaimId.set(event.claimId, event);
  }

  const claims = [...firstClaimByClaimId.values()].sort(compareTotalOrder).map((claim) => {
    const terminal = firstTerminalByClaimId.get(claim.claimId);
    const state =
      terminal?.type === "complete"
        ? "completed"
        : terminal?.type === "release"
          ? "released"
          : terminal?.type === "abandon"
            ? "abandoned"
            : "active";
    const ageMs = now - Date.parse(claim.createdAt);
    return {
      claimId: claim.claimId,
      head: claim.head,
      worker: claim.worker,
      createdAt: claim.createdAt,
      databaseId: claim.databaseId,
      state,
      reclaimable: state === "active" && claim.head === head && ageMs >= staleMs,
    };
  });

  const winner =
    claims.find((claim) => claim.head === head && (claim.state === "active" || claim.state === "completed")) ?? null;

  return { head, claims, winner };
}

/** True when `head` already has a completed review and only gates need re-checking. */
export function needsGateOnlyReevaluation(election) {
  return election.winner?.state === "completed";
}

/** True when no live (active or completed) claim exists yet for this head. */
export function needsFullReview(election) {
  return election.winner === null;
}

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

const COMMENTS_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      headRefOid
      comments(first: 100, after: $after) {
        nodes { databaseId createdAt body author { login } }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}`;

/** Paginate every PR comment via an injected page fetcher, head-pinned like review-thread-gate.mjs. */
export async function collectClaimEvents(fetchPage) {
  const events = [];
  const seenCursors = new Set();
  let cursor;
  let headSha;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const payload = await fetchPage(cursor);
    const pullRequest = payload?.data?.repository?.pullRequest;
    if (!pullRequest) throw new Error("pull request not found");
    if (headSha && pullRequest.headRefOid !== headSha) throw new Error("pull request head changed during pagination");
    headSha = pullRequest.headRefOid;
    const connection = pullRequest.comments;
    if (!Array.isArray(connection?.nodes)) throw new Error("invalid comment response");
    for (const comment of connection.nodes) {
      const event = parseClaimEvent(comment);
      if (event) events.push(event);
    }
    if (!connection.pageInfo?.hasNextPage) return { headSha, events };
    cursor = connection.pageInfo.endCursor;
    if (!cursor || seenCursors.has(cursor)) throw new Error("invalid comment pagination cursor");
    seenCursors.add(cursor);
  }
  throw new Error("comment pagination exceeded 100 pages");
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function fixturePageFetcher(fixturePath) {
  const fixture = parseJson(readFileSync(fixturePath, "utf8"), "invalid claim fixture");
  let pageIndex = 0;
  return (cursor) => {
    if (pageIndex > 0) {
      const expectedCursor = fixture.pages[pageIndex - 1].data.repository.pullRequest.comments.pageInfo.endCursor;
      if (cursor !== expectedCursor) throw new Error("fixture pagination cursor mismatch");
    }
    const page = fixture.pages[pageIndex];
    pageIndex += 1;
    if (!page) throw new Error("fixture page missing");
    return page;
  };
}

function githubPageFetcher(owner, name, number) {
  return (cursor) => {
    const args = [
      "api",
      "graphql",
      "-f",
      `query=${COMMENTS_QUERY}`,
      "-F",
      `owner=${owner}`,
      "-F",
      `name=${name}`,
      "-F",
      `number=${number}`,
    ];
    if (cursor) args.push("-f", `after=${cursor}`);
    const result = spawnSync("gh", args, { encoding: "utf8", timeout: 15_000 });
    if (result.error || result.status !== 0) {
      throw new Error(result.stderr?.trim() || result.error?.message || "GitHub comment query failed");
    }
    return parseJson(result.stdout, "invalid GitHub comment response");
  };
}

async function main() {
  const repository = option("--repo");
  const number = Number(option("--pr"));
  const expectedHead = option("--expected-head");
  const fixturePath = option("--fixture");
  const staleMs = Number(option("--stale-ms") ?? 20 * 60 * 1000);
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra || !Number.isSafeInteger(number) || number <= 0 || !expectedHead) {
    throw new Error("claim requires --repo owner/name, --pr, and --expected-head");
  }
  const fetchPage = fixturePath ? fixturePageFetcher(fixturePath) : githubPageFetcher(owner, name, number);
  const collected = await collectClaimEvents(fetchPage);
  if (collected.headSha !== expectedHead) throw new Error("pull request head does not match expected SHA");
  const election = electClaims(collected.events, { head: expectedHead, now: Date.now(), staleMs });
  process.stdout.write(`${JSON.stringify({ repository, pullRequest: number, ...election })}\n`);
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({ level: "fatal", event: "claim_election_failed", message: error.message })}\n`,
    );
    process.exit(1);
  }
}
