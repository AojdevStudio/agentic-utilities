import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  backoffDelayMs,
  createStopSignal,
  fingerprintGateEvidence,
  initialBackoff,
  initialObservation,
  interruptedReviewDecision,
  nextAction,
  observationChanged,
  recordError,
  resetBackoffOnActivity,
  sleepUnlessStopped,
  stepBackoff,
  terminalStatus,
  updateObservation,
} from "./poll-state.mjs";

// --- backoff ---

test("backoff steps up on each idle poll and caps at the last step", () => {
  let state = initialBackoff();
  const steps = [];
  for (let i = 0; i < 6; i += 1) {
    state = stepBackoff(state);
    steps.push(state.stepIndex);
  }
  assert.deepEqual(steps, [1, 2, 3, 3, 3, 3]);
});

test("backoff resets to the shortest interval after activity", () => {
  const busy = stepBackoff(stepBackoff(stepBackoff(initialBackoff())));
  assert.equal(resetBackoffOnActivity(busy).stepIndex, 0);
});

test("recordError aborts once consecutiveErrors reaches the bound", () => {
  let state = initialBackoff();
  state = recordError(state, 3);
  assert.equal(state.aborted, false);
  state = recordError(state, 3);
  assert.equal(state.aborted, false);
  state = recordError(state, 3);
  assert.equal(state.aborted, true);
});

test("backoff delay includes bounded jitter and is never negative", () => {
  const state = { stepIndex: 0 };
  assert.equal(backoffDelayMs(state, { randomFn: () => 0.5 }), 30_000); // midpoint: zero jitter
  const max = backoffDelayMs(state, { randomFn: () => 1 });
  const min = backoffDelayMs(state, { randomFn: () => 0 });
  assert.ok(max <= 30_000 * 1.2 + 1);
  assert.ok(min >= 30_000 * 0.8 - 1);
  assert.ok(min >= 0);
});

// --- named bug: positive jitter must never push the delay past the
// documented cap (the audit's cited "5-minute cap actually reaches 6
// minutes" defect) ---

test("backoffDelayMs clamps positive jitter at the final step so the documented cap is never exceeded", () => {
  const state = { stepIndex: 3 }; // the 300_000ms (5 min) cap step
  const delay = backoffDelayMs(state, { randomFn: () => 1 }); // maximum positive jitter
  assert.ok(delay <= 300_000, `expected delay clamped to the 5-minute cap, got ${delay}ms`);
});

// --- named bug: a successful idle poll must reset consecutiveErrors, not
// just resetBackoffOnActivity (the audit's "idle success doesn't reset
// consecutive errors" defect) ---

test("stepBackoff resets consecutiveErrors on any successful poll, even an idle one, while still advancing the step", () => {
  let state = initialBackoff();
  state = recordError(state, 5);
  state = recordError(state, 5);
  assert.equal(state.consecutiveErrors, 2);
  state = stepBackoff(state);
  assert.equal(state.consecutiveErrors, 0, "an idle successful poll must reset consecutive errors");
  assert.equal(state.stepIndex, 1, "backoff still advances on an idle poll, unlike resetBackoffOnActivity");
});

// --- stop primitive ---

test("stop signal is idempotent and records the first reason", () => {
  const stop = createStopSignal();
  assert.equal(stop.shouldStop(), false);
  stop.requestStop("shutdown");
  stop.requestStop("ignored second reason");
  assert.equal(stop.shouldStop(), true);
  assert.equal(stop.reason(), "shutdown");
});

test("sleepUnlessStopped returns false immediately if already stopped, without sleeping", async () => {
  const stop = createStopSignal();
  stop.requestStop("pre-stopped");
  let slept = false;
  const result = await sleepUnlessStopped(1000, stop, async () => {
    slept = true;
  });
  assert.equal(result, false);
  assert.equal(slept, false, "must not sleep once already stopped");
});

test("sleepUnlessStopped checks the stop signal again after sleeping (mid-sleep stop)", async () => {
  const stop = createStopSignal();
  const result = await sleepUnlessStopped(1000, stop, async () => {
    stop.requestStop("stopped during sleep");
  });
  assert.equal(result, false, "a stop requested during the sleep must be observed on return");
});

test("sleepUnlessStopped returns true when nothing requested a stop", async () => {
  const stop = createStopSignal();
  const result = await sleepUnlessStopped(1000, stop, async () => {});
  assert.equal(result, true);
});

// --- named bug: a stop requested mid-sleep must be observed immediately,
// not only after the full sleep duration has already elapsed ---

test("sleepUnlessStopped is interruptible mid-sleep instead of waiting out the full timer", async () => {
  const stop = createStopSignal();
  let sleepFired = false;
  const longSleep = (ms) =>
    new Promise((resolve) => {
      setTimeout(() => {
        sleepFired = true;
        resolve();
      }, ms);
    });
  const pending = sleepUnlessStopped(2000, stop, longSleep);
  setTimeout(() => stop.requestStop("interrupt"), 5);
  const result = await pending;
  assert.equal(result, false);
  assert.equal(sleepFired, false, "must return as soon as stopped, without waiting for the full 2000ms sleep");
});

// --- interrupted review + terminal status ---

test("interruptedReviewDecision continues when no stop was requested", () => {
  assert.deepEqual(interruptedReviewDecision({ stopRequested: false, headChangedDuringReview: false }), {
    action: "continue",
  });
});

test("interruptedReviewDecision finishes an in-flight review for the same head", () => {
  const decision = interruptedReviewDecision({ stopRequested: true, headChangedDuringReview: false });
  assert.equal(decision.action, "finish");
});

test("interruptedReviewDecision aborts when the head moved mid-review", () => {
  const decision = interruptedReviewDecision({ stopRequested: true, headChangedDuringReview: true });
  assert.equal(decision.action, "abort");
});

test("terminalStatus carries reason, review count, last error, and timestamp", () => {
  const status = terminalStatus({
    reason: "control pane stop",
    reviewsCompleted: 4,
    lastError: null,
    timestamp: "2026-07-20T00:00:00Z",
  });
  assert.deepEqual(status, {
    event: "terminal_status",
    reason: "control pane stop",
    reviewsCompleted: 4,
    lastError: null,
    timestamp: "2026-07-20T00:00:00Z",
  });
});

// --- nextAction: full-review vs gate-only vs skip vs reclaim ---

test("nextAction requires a full review when no claim exists yet", () => {
  assert.equal(nextAction({ winner: null }, false), "full-review");
});

test("nextAction skips a head another worker is actively (and freshly) reviewing", () => {
  const election = { winner: { state: "active", reclaimable: false } };
  assert.equal(nextAction(election, false), "skip");
});

test("nextAction reclaims a stale active claim instead of waiting forever", () => {
  const election = { winner: { state: "active", reclaimable: true } };
  assert.equal(nextAction(election, false), "reclaim-and-full-review");
});

test("nextAction does gate-only revalidation for a completed head whose gates changed", () => {
  const election = { winner: { state: "completed" } };
  assert.equal(nextAction(election, true), "gate-only");
  assert.equal(nextAction(election, false), "skip");
});

test("nextAction requires a full review for a released or abandoned claim", () => {
  assert.equal(nextAction({ winner: { state: "released" } }, false), "full-review");
  assert.equal(nextAction({ winner: { state: "abandoned" } }, false), "full-review");
});

// --- gate-evidence fingerprinting ---

function evidence(overrides = {}) {
  return {
    threads: [{ id: "t1", isResolved: false, isOutdated: false, automated: false }],
    checks: [
      { name: "lint", state: "pass", required: true },
      { name: "advisory/coverage", state: "pending", required: false },
    ],
    ...overrides,
  };
}

test("fingerprintGateEvidence is stable across GraphQL response reordering", () => {
  const a = evidence();
  const b = evidence({ checks: [...evidence().checks].reverse() });
  assert.equal(fingerprintGateEvidence(a), fingerprintGateEvidence(b));
});

test("fingerprintGateEvidence changes when an individual check's state flips, even if overall stays the same", () => {
  const before = fingerprintGateEvidence(evidence());
  const after = fingerprintGateEvidence(evidence({ checks: [{ name: "lint", state: "fail", required: true }] }));
  assert.notEqual(before, after);
});

test("fingerprintGateEvidence changes when an advisory (non-required) check's state changes", () => {
  const before = fingerprintGateEvidence(evidence());
  const after = fingerprintGateEvidence(
    evidence({ checks: [evidence().checks[0], { name: "advisory/coverage", state: "pass", required: false }] }),
  );
  assert.notEqual(before, after);
});

test("fingerprintGateEvidence changes when a thread's human/automated classification changes", () => {
  const before = fingerprintGateEvidence(evidence());
  const after = fingerprintGateEvidence(
    evidence({ threads: [{ id: "t1", isResolved: false, isOutdated: false, automated: true }] }),
  );
  assert.notEqual(before, after);
});

// --- observation persistence ---

test("initialObservation starts with no head and no state", () => {
  assert.deepEqual(initialObservation(35), {
    pr: 35,
    head: null,
    updatedAt: null,
    gateFingerprint: null,
    claimState: null,
  });
});

test("updateObservation folds a gate-evidence fingerprint and claim evidence into the persisted record", () => {
  const previous = initialObservation(35);
  const gateEvidence = evidence();
  const election = { winner: { claimId: "c1", state: "active" } };
  const next = updateObservation(previous, { head: "abc123", gateEvidence, election, now: "2026-07-20T00:00:00Z" });
  assert.deepEqual(next, {
    pr: 35,
    head: "abc123",
    updatedAt: "2026-07-20T00:00:00Z",
    gateFingerprint: fingerprintGateEvidence(gateEvidence),
    claimState: { claimId: "c1", state: "active" },
  });
});

test("observationChanged detects a head change, a fingerprint change, and no change", () => {
  const previous = { head: "abc123", gateFingerprint: fingerprintGateEvidence(evidence()), claimState: null };
  assert.equal(observationChanged(null, previous), true, "no prior observation counts as changed");
  assert.equal(observationChanged(previous, { ...previous }), false);
  assert.equal(observationChanged(previous, { ...previous, head: "def456" }), true);
  const changedFingerprint = fingerprintGateEvidence(
    evidence({ checks: [{ name: "lint", state: "fail", required: true }] }),
  );
  assert.equal(observationChanged(previous, { ...previous, gateFingerprint: changedFingerprint }), true);
});

test("observationChanged is a same-overall/different-check transition detector: overall unchanged, one check flipped", () => {
  // Both snapshots would summarize to overall="fail" (lint failing dominates),
  // but which check is failing is different — the fingerprint must still
  // register this as a change so a same-overall gate-only re-poll isn't
  // silently skipped.
  const before = {
    head: "abc123",
    gateFingerprint: fingerprintGateEvidence(
      evidence({
        checks: [
          { name: "lint", state: "fail", required: true },
          { name: "test", state: "pass", required: true },
        ],
      }),
    ),
    claimState: null,
  };
  const after = {
    ...before,
    gateFingerprint: fingerprintGateEvidence(
      evidence({
        checks: [
          { name: "lint", state: "pass", required: true },
          { name: "test", state: "fail", required: true },
        ],
      }),
    ),
  };
  assert.equal(observationChanged(before, after), true);
});

// --- self-test CLI ---

const scriptPath = fileURLToPath(new URL("./poll-state.mjs", import.meta.url));

test("poll-state --self-test exits 0 and reports a pass status", () => {
  const result = Bun.spawnSync(["bun", scriptPath, "--self-test"]);
  assert.equal(result.exitCode, 0);
  const status = JSON.parse(new TextDecoder().decode(result.stdout));
  assert.equal(status.status, "pass");
});
