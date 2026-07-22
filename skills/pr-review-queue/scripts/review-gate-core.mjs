const AUTOMATED_LOGINS = new Set(["coderabbitai", "dependabot", "github-actions", "copilot"]);

export function isAutomatedAuthor(author) {
  if (!author) return false;
  if (author.__typename === "Bot") return true;
  const login = (author.login ?? "").toLowerCase();
  return login.endsWith("[bot]") || AUTOMATED_LOGINS.has(login);
}

export async function collectReviewThreads(fetchPage) {
  const threads = [];
  const seenCursors = new Set();
  let cursor;
  let headSha;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const pullRequest = (await fetchPage(cursor))?.data?.repository?.pullRequest;
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

const CHECK_RUN_CONCLUSIONS = {
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
const CHECK_RUN_STATUSES = {
  QUEUED: "pending",
  IN_PROGRESS: "pending",
  PENDING: "pending",
  WAITING: "pending",
  REQUESTED: "pending",
};
const STATUS_CONTEXT_STATES = {
  SUCCESS: "pass",
  FAILURE: "fail",
  ERROR: "error",
  PENDING: "pending",
  EXPECTED: "pending",
};

export function classifyCheckNode(node) {
  if (node.__typename === "CheckRun") {
    const state =
      node.status === "COMPLETED" ? CHECK_RUN_CONCLUSIONS[node.conclusion] : CHECK_RUN_STATUSES[node.status];
    if (!state) throw new Error(`unknown check-run state: ${node.status}/${node.conclusion}`);
    return { name: node.name, state };
  }
  if (node.__typename === "StatusContext") {
    const state = STATUS_CONTEXT_STATES[node.state];
    if (!state) throw new Error(`unknown status-context state: ${node.state}`);
    return { name: node.context, state };
  }
  throw new Error(`unknown check node type: ${node.__typename}`);
}

export async function collectCheckContexts(fetchPage) {
  const checks = [];
  const seenCursors = new Set();
  let cursor;
  let headSha;
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const pullRequest = (await fetchPage(cursor))?.data?.repository?.pullRequest;
    if (!pullRequest) throw new Error("pull request not found for check query");
    if (headSha && pullRequest.headRefOid !== headSha) throw new Error("pull request head changed during pagination");
    headSha = pullRequest.headRefOid;
    const connection = pullRequest.commits?.nodes?.[0]?.commit?.statusCheckRollup?.contexts;
    if (!Array.isArray(connection?.nodes)) throw new Error("invalid check context response");
    checks.push(...connection.nodes);
    if (!connection.pageInfo?.hasNextPage) return { headSha, checks: checks.map(classifyCheckNode) };
    cursor = connection.pageInfo.endCursor;
    if (!cursor || seenCursors.has(cursor)) throw new Error("invalid check context pagination cursor");
    seenCursors.add(cursor);
  }
  throw new Error("check context pagination exceeded 100 pages");
}

export function parseRequiredNames(payload) {
  return payload?.data?.repository?.pullRequest?.baseRef?.branchProtectionRule?.requiredStatusCheckContexts ?? [];
}

const SEVERITY = ["error", "fail", "cancelled", "pending", "unavailable", "pass"];
const worstState = (states) => SEVERITY.find((state) => states.includes(state)) ?? "unavailable";

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

export function gateEvidence(threadResult, checkResult, now) {
  if (threadResult.headSha !== checkResult.headSha) {
    throw new Error("review threads and check state were read for different heads");
  }
  const threads = reviewThreadEvidence(threadResult);
  const gates = summarizeGates(checkResult.checks, checkResult.requiredNames);
  const blocking = (automated) =>
    threads.threads
      .filter((thread) => !thread.isResolved && !thread.isOutdated && thread.automated === automated)
      .map((thread) => thread.id);
  return {
    schemaVersion: 1,
    headSha: threadResult.headSha,
    checkedAt: now,
    threads: threads.threads,
    blockingThreadIds: threads.blockingThreadIds,
    humanBlockingThreadIds: blocking(false),
    automatedBlockingThreadIds: blocking(true),
    checks: gates.checks,
    overall: gates.overall,
  };
}

export function assertHeadUnchanged(evidence, expectedHead) {
  if (evidence.headSha !== expectedHead) {
    throw new Error(`head changed since gate evidence was gathered: expected ${expectedHead}, got ${evidence.headSha}`);
  }
  return evidence;
}
