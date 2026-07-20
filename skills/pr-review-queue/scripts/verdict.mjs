#!/usr/bin/env bun
// Versioned verdict contract. Replaces the prior free-form line-oriented
// VERDICT block with a strict, parseable JSON object so the orchestrator
// never has to regex-scrape prose to decide what to do next.

export const VERDICT_SCHEMA_VERSION = 1;
const GATE_EVIDENCE_SCHEMA_VERSION = 1;
const CI_OVERALL_STATES = new Set(["pass", "fail", "pending", "cancelled", "unavailable", "error"]);

const STATUS_VALUES = new Set(["MERGE_READY", "NEEDS_WORK", "BLOCKED"]);
// Only full-review is valid this release. mechanical-delta-ack is removed:
// every head change requires a fresh full review and a current-head
// verdict. The field stays machine-readable so a future release can widen
// this set once a mechanically-proven carry-forward implementation exists.
const SYNC_VALUES = new Set(["full-review"]);

/**
 * Validate the shape of a gate-evidence object (review-gate.mjs's
 * gateEvidence() output), not merely that it's some object. A verdict
 * carrying an empty, malformed, or wrong-shaped gates payload is rejected
 * before it can ever be read as evidence of anything.
 */
export function validateGateEvidence(gates) {
  if (!gates || typeof gates !== "object") return ["gates must be an object"];
  const errors = [];
  if (gates.schemaVersion !== GATE_EVIDENCE_SCHEMA_VERSION) {
    errors.push(`gates.schemaVersion must be ${GATE_EVIDENCE_SCHEMA_VERSION}`);
  }
  if (typeof gates.headSha !== "string" || !/^[0-9a-f]{7,40}$/i.test(gates.headSha)) {
    errors.push("gates.headSha must be a git SHA (7-40 hex characters)");
  }
  if (!CI_OVERALL_STATES.has(gates.overall)) {
    errors.push(`gates.overall must be one of ${[...CI_OVERALL_STATES].join(", ")}`);
  }
  if (!Array.isArray(gates.checks)) errors.push("gates.checks must be an array");
  if (!Array.isArray(gates.blockingThreadIds)) errors.push("gates.blockingThreadIds must be an array");
  if (!Array.isArray(gates.humanBlockingThreadIds)) errors.push("gates.humanBlockingThreadIds must be an array");
  if (!Array.isArray(gates.automatedBlockingThreadIds))
    errors.push("gates.automatedBlockingThreadIds must be an array");
  return errors;
}

/** Validate a parsed verdict object; returns an array of error strings (empty = valid). */
export function validateVerdict(verdict) {
  const errors = [];
  if (!verdict || typeof verdict !== "object") return ["verdict must be an object"];

  if (verdict.schemaVersion !== VERDICT_SCHEMA_VERSION) {
    errors.push(`schemaVersion must be ${VERDICT_SCHEMA_VERSION}`);
  }
  if (typeof verdict.repository !== "string" || !/^[^/\s]+\/[^/\s]+$/.test(verdict.repository)) {
    errors.push("repository must be 'owner/name'");
  }
  if (!Number.isSafeInteger(verdict.pullRequest) || verdict.pullRequest <= 0) {
    errors.push("pullRequest must be a positive integer");
  }
  if (typeof verdict.head !== "string" || !/^[0-9a-f]{7,40}$/i.test(verdict.head)) {
    errors.push("head must be a git SHA (7-40 hex characters)");
  }
  if (!STATUS_VALUES.has(verdict.status)) {
    errors.push(`status must be one of ${[...STATUS_VALUES].join(", ")}`);
  }
  if (!SYNC_VALUES.has(verdict.sync)) {
    errors.push(`sync must be one of ${[...SYNC_VALUES].join(", ")} in this release`);
  }
  if (typeof verdict.timestamp !== "string" || Number.isNaN(Date.parse(verdict.timestamp))) {
    errors.push("timestamp must be an ISO 8601 string");
  }
  for (const gateError of validateGateEvidence(verdict.gates)) errors.push(`gates.${gateError}`);
  if (
    verdict.gates &&
    typeof verdict.gates.headSha === "string" &&
    typeof verdict.head === "string" &&
    verdict.gates.headSha !== verdict.head
  ) {
    errors.push("gates.headSha must equal the verdict's head");
  }

  const blocking = Array.isArray(verdict.blocking) ? verdict.blocking : null;
  if (!blocking) errors.push("blocking must be an array of finding IDs");
  else if (blocking.some((id) => typeof id !== "string" || !id))
    errors.push("blocking entries must be non-empty strings");

  const nonBlocking = Array.isArray(verdict.nonBlocking) ? verdict.nonBlocking : null;
  if (!nonBlocking) errors.push("nonBlocking must be an array of finding IDs");
  else if (nonBlocking.some((id) => typeof id !== "string" || !id))
    errors.push("nonBlocking entries must be non-empty strings");

  if (blocking) {
    const seen = new Set(blocking);
    if (seen.size !== blocking.length) errors.push("blocking IDs must be unique");
    if (nonBlocking) {
      const overlap = blocking.filter((id) => nonBlocking.includes(id));
      if (overlap.length > 0)
        errors.push(`finding IDs cannot be both blocking and non-blocking: ${overlap.join(", ")}`);
    }
  }
  if (nonBlocking && new Set(nonBlocking).size !== nonBlocking.length) errors.push("nonBlocking IDs must be unique");

  if (verdict.status === "NEEDS_WORK") {
    if (typeof verdict.fixScope !== "string" || !verdict.fixScope.trim()) {
      errors.push("fixScope is required when status is NEEDS_WORK");
    }
    if (blocking && blocking.length === 0) errors.push("NEEDS_WORK requires at least one blocking finding ID");
  }
  if (verdict.status === "BLOCKED") {
    if (typeof verdict.blockedReason !== "string" || !verdict.blockedReason.trim()) {
      errors.push("blockedReason is required when status is BLOCKED");
    }
  }
  if (verdict.status === "MERGE_READY") {
    if (blocking && blocking.length > 0) errors.push("MERGE_READY must have zero blocking findings");
    if (verdict.gates && verdict.gates.overall !== "pass") {
      errors.push("MERGE_READY requires gates.overall to be pass");
    }
    if (verdict.gates && Array.isArray(verdict.gates.blockingThreadIds) && verdict.gates.blockingThreadIds.length > 0) {
      errors.push("MERGE_READY requires zero unresolved blocking review threads in gates");
    }
  }

  return errors;
}

/** Parse and validate a verdict from JSON text. Throws with every error joined, never partially. */
export function parseVerdict(text) {
  let data;
  try {
    data = JSON.parse(text);
  } catch (error) {
    throw new Error(`verdict is not valid JSON: ${error.message}`);
  }
  const errors = validateVerdict(data);
  if (errors.length > 0) throw new Error(`invalid verdict: ${errors.join("; ")}`);
  return data;
}

/** True when a previously posted verdict no longer matches the PR's current head. */
export function isStaleVerdict(verdict, currentHead) {
  return verdict.head !== currentHead;
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

async function main() {
  const text = option("--file") ? await Bun.file(option("--file")).text() : await Bun.stdin.text();
  const verdict = parseVerdict(text);
  const currentHead = option("--current-head");
  if (currentHead && isStaleVerdict(verdict, currentHead)) {
    process.stdout.write(`${JSON.stringify({ ...verdict, stale: true })}\n`);
    process.exitCode = 4;
    return;
  }
  process.stdout.write(`${JSON.stringify({ ...verdict, stale: false })}\n`);
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ level: "fatal", event: "verdict_invalid", message: error.message })}\n`);
    process.exit(1);
  }
}
