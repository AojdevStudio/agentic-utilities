#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

// Head-pinned, paginated gate evidence: review threads (human vs automated,
// resolved/outdated/blocking) and CI check states (pass/fail/pending/
// cancelled/unavailable/error, required vs advisory), tied to one exact
// reviewed head and one collection timestamp. Extends the pattern in
// skills/herdr-fleet/scripts/review-thread-gate.mjs with author
// classification and CI rollup; kept self-contained here rather than
// importing across skills so pr-review-queue stays installable alone.

const GATE_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      headRefOid
      reviewThreads(first: 100, after: $after) {
        nodes {
          id
          isResolved
          isOutdated
          comments(first: 1) { nodes { url author { login __typename } } }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}`;

const CHECKS_QUERY = `query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      headRefOid
      baseRefName
      commits(last: 1) {
        nodes {
          commit {
            statusCheckRollup {
              state
              contexts(first: 100) {
                nodes {
                  __typename
                  ... on CheckRun { name status conclusion }
                  ... on StatusContext { context state }
                }
              }
            }
          }
        }
      }
    }
  }
  branchProtection: repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      baseRef {
        branchProtectionRule { requiredStatusCheckContexts }
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

const AUTOMATED_LOGIN = /coderabbit|dependabot|github-actions|copilot|\[bot\]$/i;

/** True when a comment/check author is automated review tooling rather than a human reviewer. */
export function isAutomatedAuthor(author) {
  if (!author) return false;
  if (author.__typename === "Bot") return true;
  return AUTOMATED_LOGIN.test(author.login ?? "");
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
    const comment = thread.comments?.nodes?.[0];
    if (!thread.id || !comment?.url) throw new Error("review thread is missing ID or URL");
    return {
      id: thread.id,
      url: comment.url,
      isResolved: thread.isResolved === true,
      isOutdated: thread.isOutdated === true,
      automated: isAutomatedAuthor(comment.author),
    };
  });
  return {
    headSha: result.headSha,
    threads,
    blockingThreadIds: threads.filter((thread) => !thread.isResolved && !thread.isOutdated).map((thread) => thread.id),
  };
}

const CHECK_RUN_CONCLUSION_STATE = {
  SUCCESS: "pass",
  NEUTRAL: "pass",
  SKIPPED: "pass",
  FAILURE: "fail",
  TIMED_OUT: "fail",
  ACTION_REQUIRED: "fail",
  STALE: "fail",
  CANCELLED: "cancelled",
};

const CHECK_RUN_STATUS_STATE = {
  QUEUED: "pending",
  IN_PROGRESS: "pending",
  PENDING: "pending",
  WAITING: "pending",
  REQUESTED: "pending",
};

const STATUS_CONTEXT_STATE = {
  SUCCESS: "pass",
  FAILURE: "fail",
  ERROR: "error",
  PENDING: "pending",
  EXPECTED: "pending",
};

/** Map one GraphQL check-rollup context node to { name, state }. */
export function classifyCheckNode(node) {
  if (node.__typename === "CheckRun") {
    if (node.status === "COMPLETED") {
      const state = CHECK_RUN_CONCLUSION_STATE[node.conclusion];
      if (!state) throw new Error(`unknown check-run conclusion: ${node.conclusion}`);
      return { name: node.name, state };
    }
    const state = CHECK_RUN_STATUS_STATE[node.status];
    if (!state) throw new Error(`unknown check-run status: ${node.status}`);
    return { name: node.name, state };
  }
  if (node.__typename === "StatusContext") {
    const state = STATUS_CONTEXT_STATE[node.state];
    if (!state) throw new Error(`unknown status-context state: ${node.state}`);
    return { name: node.context, state };
  }
  throw new Error(`unknown check node type: ${node.__typename}`);
}

const SEVERITY_WORST_FIRST = ["error", "fail", "cancelled", "pending", "unavailable", "pass"];

function worstState(states) {
  for (const candidate of SEVERITY_WORST_FIRST) if (states.includes(candidate)) return candidate;
  return "unavailable";
}

/**
 * Summarize a flat list of { name, state } checks against the branch's
 * required-status-check names. Only required checks affect `overall`;
 * advisory checks are reported but never block.
 */
export function summarizeGates(checks, requiredNames) {
  const named = checks.map((check) => ({ ...check, required: requiredNames.includes(check.name) }));
  const requiredStates = named.filter((check) => check.required).map((check) => check.state);
  return { checks: named, overall: requiredStates.length === 0 ? "unavailable" : worstState(requiredStates) };
}

/** Parse the raw CHECKS_QUERY response into { headSha, checks, requiredNames }. */
export function parseCheckPayload(payload) {
  const pullRequest = payload?.data?.repository?.pullRequest;
  if (!pullRequest) throw new Error("pull request not found for check query");
  const rollup = pullRequest.commits?.nodes?.[0]?.commit?.statusCheckRollup;
  const nodes = rollup?.contexts?.nodes ?? [];
  const checks = nodes.map(classifyCheckNode);
  const requiredNames =
    payload?.data?.branchProtection?.pullRequest?.baseRef?.branchProtectionRule?.requiredStatusCheckContexts ?? [];
  return { headSha: pullRequest.headRefOid, checks, requiredNames };
}

/** Combine thread evidence and gate evidence into one head-pinned, timestamped result. */
export function gateEvidence(threadResult, checkResult, now) {
  if (threadResult.headSha !== checkResult.headSha) {
    throw new Error("review threads and check state were read for different heads");
  }
  const threads = reviewThreadEvidence(threadResult);
  const gates = summarizeGates(checkResult.checks, checkResult.requiredNames);
  return {
    headSha: threadResult.headSha,
    checkedAt: now,
    threads: threads.threads,
    blockingThreadIds: threads.blockingThreadIds,
    humanBlockingThreadIds: threads.threads
      .filter((thread) => !thread.isResolved && !thread.isOutdated && !thread.automated)
      .map((thread) => thread.id),
    automatedBlockingThreadIds: threads.threads
      .filter((thread) => !thread.isResolved && !thread.isOutdated && thread.automated)
      .map((thread) => thread.id),
    gates,
  };
}

/** True when `evidence` is safe to act on: gates and threads at exactly `expectedHead`. */
export function assertHeadUnchanged(evidence, expectedHead) {
  if (evidence.headSha !== expectedHead) {
    throw new Error(`head changed since gate evidence was gathered: expected ${expectedHead}, got ${evidence.headSha}`);
  }
  return evidence;
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function fixtureThreadPageFetcher(fixturePath) {
  const fixture = parseJson(readFileSync(fixturePath, "utf8"), "invalid gate fixture");
  let pageIndex = 0;
  return {
    fetchThreadPage: (cursor) => {
      if (pageIndex > 0) {
        const expectedCursor =
          fixture.threadPages[pageIndex - 1].data.repository.pullRequest.reviewThreads.pageInfo.endCursor;
        if (cursor !== expectedCursor) throw new Error("fixture pagination cursor mismatch");
      }
      const page = fixture.threadPages[pageIndex];
      pageIndex += 1;
      if (!page) throw new Error("fixture thread page missing");
      return page;
    },
    checkPayload: fixture.checks,
  };
}

function githubFetchers(owner, name, number) {
  function run(query, extra = []) {
    const args = [
      "api",
      "graphql",
      "-f",
      `query=${query}`,
      "-F",
      `owner=${owner}`,
      "-F",
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
  }
  return {
    fetchThreadPage: (cursor) => run(GATE_QUERY, cursor ? ["-f", `after=${cursor}`] : []),
    checkPayload: run(CHECKS_QUERY),
  };
}

async function main() {
  const repository = option("--repo");
  const number = Number(option("--pr"));
  const expectedHead = option("--expected-head");
  const fixturePath = option("--fixture");
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra || !Number.isSafeInteger(number) || number <= 0 || !expectedHead) {
    throw new Error("review-gate requires --repo owner/name, --pr, and --expected-head");
  }
  const { fetchThreadPage, checkPayload } = fixturePath
    ? fixtureThreadPageFetcher(fixturePath)
    : githubFetchers(owner, name, number);
  const threadResult = await collectReviewThreads(fetchThreadPage);
  const checkResult = parseCheckPayload(checkPayload);
  const evidence = gateEvidence(threadResult, checkResult, new Date().toISOString());
  assertHeadUnchanged(evidence, expectedHead);
  process.stdout.write(`${JSON.stringify({ repository, pullRequest: number, ...evidence })}\n`);
  if (evidence.blockingThreadIds.length > 0 || evidence.gates.overall !== "pass") process.exitCode = 3;
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
