import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  backoffDelayMs,
  createStopSignal,
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

// --- observation persistence ---

test("initialObservation starts with no head and no state", () => {
  assert.deepEqual(initialObservation(35), {
    pr: 35,
    head: null,
    updatedAt: null,
    threadState: null,
    claimState: null,
  });
});

test("updateObservation folds gate and claim evidence into the persisted record", () => {
  const previous = initialObservation(35);
  const gateEvidence = { blockingThreadIds: ["t1"], gates: { overall: "fail" } };
  const election = { winner: { claimId: "c1", state: "active" } };
  const next = updateObservation(previous, { head: "abc123", gateEvidence, election, now: "2026-07-20T00:00:00Z" });
  assert.deepEqual(next, {
    pr: 35,
    head: "abc123",
    updatedAt: "2026-07-20T00:00:00Z",
    threadState: { blockingThreadIds: ["t1"], gatesOverall: "fail" },
    claimState: { claimId: "c1", state: "active" },
  });
});

test("observationChanged detects a head change, a gate change, and no change", () => {
  const previous = { head: "abc123", threadState: { blockingThreadIds: [] }, claimState: null };
  assert.equal(observationChanged(null, previous), true, "no prior observation counts as changed");
  assert.equal(observationChanged(previous, { ...previous }), false);
  assert.equal(observationChanged(previous, { ...previous, head: "def456" }), true);
  assert.equal(observationChanged(previous, { ...previous, threadState: { blockingThreadIds: ["t1"] } }), true);
});

// --- self-test CLI ---

const scriptPath = fileURLToPath(new URL("./poll-state.mjs", import.meta.url));

test("poll-state --self-test exits 0 and reports a pass status", () => {
  const result = Bun.spawnSync(["bun", scriptPath, "--self-test"]);
  assert.equal(result.exitCode, 0);
  const status = JSON.parse(new TextDecoder().decode(result.stdout));
  assert.equal(status.status, "pass");
});
