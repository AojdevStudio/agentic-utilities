#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import {
  assertHeadUnchanged,
  collectCheckContexts,
  collectReviewThreads,
  gateEvidence,
  parseRequiredNames,
} from "./review-gate-core.mjs";

export * from "./review-gate-core.mjs";

const THREADS_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) {
    headRefOid
    reviewThreads(first: 100, after: $after) {
      nodes { id isResolved isOutdated comments(first: 1) { nodes { url author { login __typename } } } }
      pageInfo { hasNextPage endCursor }
    }
  } }
}`;
const CHECKS_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) {
    headRefOid
    commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 100, after: $after) {
      nodes { __typename ... on CheckRun { name status conclusion } ... on StatusContext { context state } }
      pageInfo { hasNextPage endCursor }
    } } } } }
  } }
}`;
const REQUIRED_QUERY = `query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) {
    baseRef { branchProtectionRule { requiredStatusCheckContexts } }
  } }
}`;

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function fixtureFetchers(path) {
  const fixture = parseJson(readFileSync(path, "utf8"), "invalid gate fixture");
  let threadIndex = 0;
  let checkIndex = 0;
  return {
    fetchThreadPage: (cursor) => {
      if (threadIndex > 0) {
        const previous = fixture.threadPages[threadIndex - 1].data.repository.pullRequest.reviewThreads.pageInfo;
        if (cursor !== previous.endCursor) throw new Error("fixture thread pagination cursor mismatch");
      }
      const page = fixture.threadPages[threadIndex++];
      if (!page) throw new Error("fixture thread page missing");
      return page;
    },
    fetchCheckPage: (cursor) => {
      if (checkIndex > 0) {
        const commit = fixture.checkPages[checkIndex - 1].data.repository.pullRequest.commits.nodes[0].commit;
        if (cursor !== commit.statusCheckRollup.contexts.pageInfo.endCursor) {
          throw new Error("fixture check pagination cursor mismatch");
        }
      }
      const page = fixture.checkPages[checkIndex++];
      if (!page) throw new Error("fixture check page missing");
      return page;
    },
    requiredNamesPayload: fixture.requiredNames,
  };
}

function githubFetchers(owner, name, number) {
  const run = (query, extra = []) => {
    const args = [
      "api",
      "graphql",
      "-f",
      `query=${query}`,
      "-f",
      `owner=${owner}`,
      "-f",
      `name=${name}`,
      "-F",
      `number=${number}`,
      ...extra,
    ];
    const result = spawnSync("gh", args, { encoding: "utf8", timeout: 15_000 });
    if (result.error || result.status !== 0) {
      throw new Error(result.stderr?.trim() || result.error?.message || "GitHub gate query failed");
    }
    return parseJson(result.stdout, "invalid GitHub gate response");
  };
  return {
    fetchThreadPage: (cursor) => run(THREADS_QUERY, cursor ? ["-f", `after=${cursor}`] : []),
    fetchCheckPage: (cursor) => run(CHECKS_QUERY, cursor ? ["-f", `after=${cursor}`] : []),
    requiredNamesPayload: run(REQUIRED_QUERY),
  };
}

function cliOptions() {
  const repository = option("--repo");
  const number = Number(option("--pr"));
  const expectedHead = option("--expected-head");
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra || !Number.isSafeInteger(number) || number <= 0 || !expectedHead) {
    throw new Error("review-gate requires --repo owner/name, --pr, and --expected-head");
  }
  return { repository, owner, name, number, expectedHead, fixturePath: option("--fixture") };
}

async function main() {
  const options = cliOptions();
  const fetchers = options.fixturePath
    ? fixtureFetchers(options.fixturePath)
    : githubFetchers(options.owner, options.name, options.number);
  const threadResult = await collectReviewThreads(fetchers.fetchThreadPage);
  const checkResult = await collectCheckContexts(fetchers.fetchCheckPage);
  const requiredNames = parseRequiredNames(fetchers.requiredNamesPayload);
  const evidence = gateEvidence(threadResult, { ...checkResult, requiredNames }, new Date().toISOString());
  assertHeadUnchanged(evidence, options.expectedHead);
  process.stdout.write(
    `${JSON.stringify({ repository: options.repository, pullRequest: options.number, ...evidence })}\n`,
  );
  if (evidence.blockingThreadIds.length > 0 || evidence.overall !== "pass") process.exitCode = 3;
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({ level: "fatal", event: "review_gate_failed", message: error.message })}\n`,
    );
    process.exit(1);
  }
}
