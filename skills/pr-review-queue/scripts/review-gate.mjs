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

const THREADS_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
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

// Contexts are paginated separately from threads: a rollup can carry more
// than 100 check contexts (many required checks across a monorepo), and a
// single unpaginated fetch silently truncates the rest.
const CHECK_CONTEXTS_QUERY = `query($owner: String!, $name: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      headRefOid
      commits(last: 1) {
        nodes {
          commit {
            statusCheckRollup {
              contexts(first: 100, after: $after) {
                nodes {
                  __typename
                  ... on CheckRun { name status conclusion }
                  ... on StatusContext { context state }
                }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
        }
      }
    }
  }
}`;

// Required-check names come from classic branch protection. This is a
// one-shot query (the list itself doesn't paginate in practice), kept
// separate from context pagination so the two concerns don't share cursors.
const REQUIRED_NAMES_QUERY = `query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
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
  STARTUP_FAILURE: "fail",
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

/** Paginate every check-rollup context, head-pinned, until pageInfo.hasNextPage is false. */
export async function collectCheckContexts(fetchPage) {
  const nodes = [];
  const seenCursors = new Set();
  let cursor;
  let headSha;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const payload = await fetchPage(cursor);
    const pullRequest = payload?.data?.repository?.pullRequest;
    if (!pullRequest) throw new Error("pull request not found for check query");
    if (headSha && pullRequest.headRefOid !== headSha) throw new Error("pull request head changed during pagination");
    headSha = pullRequest.headRefOid;
    const connection = pullRequest.commits?.nodes?.[0]?.commit?.statusCheckRollup?.contexts;
    if (!Array.isArray(connection?.nodes)) throw new Error("invalid check context response");
    nodes.push(...connection.nodes);
    if (!connection.pageInfo?.hasNextPage) return { headSha, checks: nodes.map(classifyCheckNode) };
    cursor = connection.pageInfo.endCursor;
    if (!cursor || seenCursors.has(cursor)) throw new Error("invalid check context pagination cursor");
    seenCursors.add(cursor);
  }
  throw new Error("check context pagination exceeded 100 pages");
}

/** Parse the one-shot REQUIRED_NAMES_QUERY response into a plain array of required check names. */
export function parseRequiredNames(payload) {
  return payload?.data?.repository?.pullRequest?.baseRef?.branchProtectionRule?.requiredStatusCheckContexts ?? [];
}

const SEVERITY_WORST_FIRST = ["error", "fail", "cancelled", "pending", "unavailable", "pass"];

function worstState(states) {
  for (const candidate of SEVERITY_WORST_FIRST) if (states.includes(candidate)) return candidate;
  return "unavailable";
}

/**
 * Summarize observed checks against the full required-name list. A
 * required name with no matching observed check is synthesized as
 * `unavailable` rather than silently dropped — otherwise a required check
 * that never reported (not merely pending) reads as if it didn't exist,
 * and an unrelated observed subset could be blessed as passing.
 */
export function summarizeGates(checks, requiredNames) {
  const observedNames = new Set(checks.map((check) => check.name));
  const synthesized = requiredNames
    .filter((name) => !observedNames.has(name))
    .map((name) => ({ name, state: "unavailable", synthesized: true }));
  const named = [...checks.map((check) => ({ ...check, synthesized: false })), ...synthesized].map((check) => ({
    ...check,
    required: requiredNames.includes(check.name),
  }));
  const requiredStates = named.filter((check) => check.required).map((check) => check.state);
  return { checks: named, overall: requiredStates.length === 0 ? "unavailable" : worstState(requiredStates) };
}

/** Combine thread evidence and gate evidence into one flat, head-pinned, timestamped result. */
export function gateEvidence(threadResult, checkResult, now) {
  if (threadResult.headSha !== checkResult.headSha) {
    throw new Error("review threads and check state were read for different heads");
  }
  const threads = reviewThreadEvidence(threadResult);
  const gates = summarizeGates(checkResult.checks, checkResult.requiredNames);
  return {
    schemaVersion: 1,
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
    checks: gates.checks,
    overall: gates.overall,
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

function fixtureFetchers(fixturePath) {
  const fixture = parseJson(readFileSync(fixturePath, "utf8"), "invalid gate fixture");
  let threadPageIndex = 0;
  let checkPageIndex = 0;
  return {
    fetchThreadPage: (cursor) => {
      if (threadPageIndex > 0) {
        const expectedCursor =
          fixture.threadPages[threadPageIndex - 1].data.repository.pullRequest.reviewThreads.pageInfo.endCursor;
        if (cursor !== expectedCursor) throw new Error("fixture thread pagination cursor mismatch");
      }
      const page = fixture.threadPages[threadPageIndex];
      threadPageIndex += 1;
      if (!page) throw new Error("fixture thread page missing");
      return page;
    },
    fetchCheckPage: (cursor) => {
      if (checkPageIndex > 0) {
        const expectedCursor =
          fixture.checkPages[checkPageIndex - 1].data.repository.pullRequest.commits.nodes[0].commit.statusCheckRollup
            .contexts.pageInfo.endCursor;
        if (cursor !== expectedCursor) throw new Error("fixture check pagination cursor mismatch");
      }
      const page = fixture.checkPages[checkPageIndex];
      checkPageIndex += 1;
      if (!page) throw new Error("fixture check page missing");
      return page;
    },
    requiredNamesPayload: fixture.requiredNames,
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
    fetchThreadPage: (cursor) => run(THREADS_QUERY, cursor ? ["-f", `after=${cursor}`] : []),
    fetchCheckPage: (cursor) => run(CHECK_CONTEXTS_QUERY, cursor ? ["-f", `after=${cursor}`] : []),
    requiredNamesPayload: run(REQUIRED_NAMES_QUERY),
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
  const { fetchThreadPage, fetchCheckPage, requiredNamesPayload } = fixturePath
    ? fixtureFetchers(fixturePath)
    : githubFetchers(owner, name, number);
  const threadResult = await collectReviewThreads(fetchThreadPage);
  const checkResult = await collectCheckContexts(fetchCheckPage);
  const requiredNames = parseRequiredNames(requiredNamesPayload);
  const evidence = gateEvidence(threadResult, { ...checkResult, requiredNames }, new Date().toISOString());
  assertHeadUnchanged(evidence, expectedHead);
  process.stdout.write(`${JSON.stringify({ repository, pullRequest: number, ...evidence })}\n`);
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
