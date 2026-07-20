#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

// The open-PR queue, paginated to exhaustion via GraphQL cursors rather
// than trusting a fixed --limit ceiling on `gh pr list` (which silently
// truncates once a repo's open-PR count exceeds whatever number was
// chosen, no matter how generous).

const QUERY = `query($owner: String!, $name: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 100, after: $after, orderBy: {field: CREATED_AT, direction: ASC}) {
      nodes { number title isDraft createdAt }
      pageInfo { hasNextPage endCursor }
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

/** Paginate every open PR via an injected page fetcher, until pageInfo.hasNextPage is false. */
export async function collectOpenPullRequests(fetchPage) {
  const pullRequests = [];
  const seenCursors = new Set();
  let cursor;
  for (let pageNumber = 0; pageNumber < 1000; pageNumber += 1) {
    const payload = await fetchPage(cursor);
    const connection = payload?.data?.repository?.pullRequests;
    if (!Array.isArray(connection?.nodes)) throw new Error("invalid pull request list response");
    pullRequests.push(...connection.nodes);
    if (!connection.pageInfo?.hasNextPage) return pullRequests;
    cursor = connection.pageInfo.endCursor;
    if (!cursor || seenCursors.has(cursor)) throw new Error("invalid pull request pagination cursor");
    seenCursors.add(cursor);
  }
  throw new Error("pull request pagination exceeded 1000 pages");
}

/** Oldest-first, non-draft PR numbers ready to be walked by PICK. */
export function reviewableQueue(pullRequests) {
  return pullRequests
    .filter((pr) => pr.isDraft !== true)
    .slice()
    .sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt))
    .map((pr) => pr.number);
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function fixturePageFetcher(fixturePath) {
  const fixture = parseJson(readFileSync(fixturePath, "utf8"), "invalid queue fixture");
  let pageIndex = 0;
  return (cursor) => {
    if (pageIndex > 0) {
      const expectedCursor = fixture.pages[pageIndex - 1].data.repository.pullRequests.pageInfo.endCursor;
      if (cursor !== expectedCursor) throw new Error("fixture pagination cursor mismatch");
    }
    const page = fixture.pages[pageIndex];
    pageIndex += 1;
    if (!page) throw new Error("fixture page missing");
    return page;
  };
}

function githubPageFetcher(owner, name) {
  return (cursor) => {
    const args = ["api", "graphql", "-f", `query=${QUERY}`, "-F", `owner=${owner}`, "-F", `name=${name}`];
    if (cursor) args.push("-f", `after=${cursor}`);
    const result = spawnSync("gh", args, { encoding: "utf8", timeout: 15_000 });
    if (result.error || result.status !== 0) {
      throw new Error(result.stderr?.trim() || result.error?.message || "GitHub pull request list query failed");
    }
    return parseJson(result.stdout, "invalid GitHub pull request list response");
  };
}

async function main() {
  const repository = option("--repo");
  const fixturePath = option("--fixture");
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra) throw new Error("queue requires --repo owner/name");
  const fetchPage = fixturePath ? fixturePageFetcher(fixturePath) : githubPageFetcher(owner, name);
  const pullRequests = await collectOpenPullRequests(fetchPage);
  const queue = reviewableQueue(pullRequests);
  process.stdout.write(`${JSON.stringify({ repository, total: pullRequests.length, queue })}\n`);
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({ level: "fatal", event: "queue_fetch_failed", message: error.message })}\n`,
    );
    process.exit(1);
  }
}
