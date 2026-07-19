#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      headRefOid
      reviewThreads(first: 100, after: $after) {
        nodes {
          id
          isResolved
          isOutdated
          comments(first: 1) { nodes { url } }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}`;

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

export async function collectReviewThreads(fetchPage) {
  const threads = [];
  const seenCursors = new Set();
  let cursor;
  let headSha;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const payload = await fetchPage(cursor);
    const pullRequest = payload?.data?.repository?.pullRequest;
    if (!pullRequest) throw new Error("pull request not found");
    if (headSha && pullRequest.headRefOid !== headSha) throw new Error("pull request head changed during pagination");
    headSha = pullRequest.headRefOid;
    const connection = pullRequest.reviewThreads;
    if (!Array.isArray(connection?.nodes)) throw new Error("invalid review thread response");
    threads.push(...connection.nodes);
    if (!connection.pageInfo?.hasNextPage) return { headSha, threads };
    cursor = connection.pageInfo.endCursor;
    if (!cursor || seenCursors.has(cursor)) throw new Error("invalid review thread pagination cursor");
    seenCursors.add(cursor);
  }
  throw new Error("review thread pagination exceeded 100 pages");
}

export function reviewThreadEvidence(result) {
  const threads = result.threads.map((thread) => {
    const url = thread.comments?.nodes?.[0]?.url;
    if (!thread.id || !url) throw new Error("review thread is missing ID or URL");
    return {
      id: thread.id,
      url,
      isResolved: thread.isResolved === true,
      isOutdated: thread.isOutdated === true,
    };
  });
  return {
    headSha: result.headSha,
    threads,
    blockingThreadIds: threads.filter((thread) => !thread.isResolved && !thread.isOutdated).map((thread) => thread.id),
  };
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function fixturePageFetcher(fixturePath) {
  const fixture = parseJson(readFileSync(fixturePath, "utf8"), "invalid review-thread fixture");
  let pageIndex = 0;
  return (cursor) => {
    if (pageIndex > 0) {
      const expectedCursor = fixture.pages[pageIndex - 1].data.repository.pullRequest.reviewThreads.pageInfo.endCursor;
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
      `query=${QUERY}`,
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
      throw new Error(result.stderr?.trim() || result.error?.message || "GitHub review-thread query failed");
    }
    return parseJson(result.stdout, "invalid GitHub review-thread response");
  };
}

async function main() {
  const repository = option("--repo");
  const number = Number(option("--pr"));
  const expectedHead = option("--expected-head");
  const fixturePath = option("--fixture");
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra || !Number.isSafeInteger(number) || number <= 0 || !expectedHead) {
    throw new Error("review-thread-gate requires --repo owner/name, --pr, and --expected-head");
  }
  const fetchPage = fixturePath ? fixturePageFetcher(fixturePath) : githubPageFetcher(owner, name, number);
  const result = await collectReviewThreads(fetchPage);
  if (result.headSha !== expectedHead) throw new Error("pull request head does not match expected SHA");
  const evidence = {
    repository,
    pullRequest: number,
    checkedAt: new Date().toISOString(),
    ...reviewThreadEvidence(result),
  };
  process.stdout.write(`${JSON.stringify(evidence)}\n`);
  if (evidence.blockingThreadIds.length > 0) process.exitCode = 3;
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({
        level: "fatal",
        event: "review_thread_gate_failed",
        sessionId: "review-thread-gate",
        message: error.message,
      })}\n`,
    );
    process.exit(1);
  }
}
