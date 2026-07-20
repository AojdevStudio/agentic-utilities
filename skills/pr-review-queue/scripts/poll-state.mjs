#!/usr/bin/env bun
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

// Polling and stop state machine for the fleet-mode reviewer lane. All
// timing/randomness is injectable so the whole thing is deterministically
// testable; the CLI entrypoint at the bottom is the only place that touches
// real wall-clock time or real sleeping.

const BACKOFF_STEPS_MS = [30_000, 60_000, 120_000, 300_000]; // 30s, 60s, 120s, capped at 5min

/**
 * A deterministic fingerprint of the FULL normalized gate evidence, not
 * just its aggregate overall+blockingThreadIds summary. Two polls with the
 * same overall verdict can still differ in which specific check flipped, an
 * advisory check's state, or a thread's human/automated classification;
 * hashing every thread and check individually (sorted, so GraphQL response
 * ordering never causes a false "changed") catches all of those.
 */
export function fingerprintGateEvidence(evidence) {
  const threads = [...(evidence.threads ?? [])]
    .map((t) => ({ id: t.id, isResolved: t.isResolved, isOutdated: t.isOutdated, automated: t.automated }))
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const checks = [...(evidence.checks ?? [])]
    .map((c) => ({ name: c.name, state: c.state, required: c.required }))
    .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return createHash("sha256").update(JSON.stringify({ threads, checks })).digest("hex");
}

/** Per-PR record persisted across polls: last-seen head, gate-evidence fingerprint, and claim state. */
export function initialObservation(pr) {
  return { pr, head: null, updatedAt: null, gateFingerprint: null, claimState: null };
}

/** Fold fresh gate/claim evidence into the persisted per-PR observation. */
export function updateObservation(previous, { head, gateEvidence, election, now }) {
  return {
    pr: previous.pr,
    head,
    updatedAt: now,
    gateFingerprint: fingerprintGateEvidence(gateEvidence),
    claimState: election.winner ? { claimId: election.winner.claimId, state: election.winner.state } : null,
  };
}

/** True when the head or the observed gate/claim state differs from what was last persisted. */
export function observationChanged(previous, next) {
  if (!previous) return true;
  return (
    previous.head !== next.head ||
    previous.gateFingerprint !== next.gateFingerprint ||
    JSON.stringify(previous.claimState) !== JSON.stringify(next.claimState)
  );
}

/**
 * Decide the next action for a PR from its claim election and whether gate
 * evidence changed since the last observation. Keeps full-review work
 * strictly separate from same-head gate-only revalidation.
 */
export function nextAction(election, gatesChanged) {
  if (election.winner === null) return "full-review";
  if (election.winner.state === "active") return election.winner.reclaimable ? "reclaim-and-full-review" : "skip";
  if (election.winner.state === "completed") return gatesChanged ? "gate-only" : "skip";
  return "full-review"; // released or abandoned: no live claim, needs a fresh full review
}

// --- backoff -----------------------------------------------------------

export function initialBackoff() {
  return { stepIndex: 0, consecutiveErrors: 0 };
}

/**
 * Step backoff up (capped at the last step) after an idle poll finds no
 * work. consecutiveErrors resets here too: an idle poll that completed
 * without error is still a successful poll, distinct from resetBackoffOnActivity
 * (which additionally collapses the backoff step back to the shortest
 * interval because real work was found).
 */
export function stepBackoff(state) {
  return { ...state, stepIndex: Math.min(state.stepIndex + 1, BACKOFF_STEPS_MS.length - 1), consecutiveErrors: 0 };
}

/** Reset backoff to the shortest interval after any poll finds real work, clearing any prior abort. */
export function resetBackoffOnActivity(state) {
  return { ...state, stepIndex: 0, consecutiveErrors: 0, aborted: false };
}

/** Record a poll failure; `aborted` becomes true once consecutiveErrors reaches maxErrors. */
export function recordError(state, maxErrors) {
  const consecutiveErrors = state.consecutiveErrors + 1;
  return { ...state, consecutiveErrors, aborted: consecutiveErrors >= maxErrors };
}

/**
 * Backoff delay with bounded jitter (+/- jitterRatio of the base step),
 * never negative and never exceeding the documented cap (the last backoff
 * step): positive jitter on top of an already-capped step must not push
 * the actual delay past what's documented as the ceiling.
 */
export function backoffDelayMs(state, { jitterRatio = 0.2, randomFn = Math.random } = {}) {
  const base = BACKOFF_STEPS_MS[state.stepIndex];
  const cap = BACKOFF_STEPS_MS[BACKOFF_STEPS_MS.length - 1];
  const jitter = base * jitterRatio * (randomFn() * 2 - 1);
  return Math.max(0, Math.min(cap, Math.round(base + jitter)));
}

// --- stop primitive ------------------------------------------------------

/**
 * An idempotent, observable stop signal checked before AND after any sleep,
 * and awaitable via whenStopped() so a sleep can be interrupted immediately
 * instead of only being noticed once its full duration has already elapsed.
 */
export function createStopSignal() {
  let stopped = false;
  let reason = null;
  let resolveStopped;
  const stoppedPromise = new Promise((resolve) => {
    resolveStopped = resolve;
  });
  return {
    requestStop(why) {
      if (stopped) return;
      stopped = true;
      reason = why ?? "stop requested";
      resolveStopped();
    },
    shouldStop() {
      return stopped;
    },
    reason() {
      return reason;
    },
    whenStopped() {
      return stoppedPromise;
    },
  };
}

/**
 * Sleep `ms`, but race it against the stop signal so a stop requested
 * mid-sleep is observed immediately rather than only after the full
 * duration elapses. False if stopped either before sleeping or by the
 * time the race settles.
 */
export async function sleepUnlessStopped(ms, stopSignal, sleepFn) {
  if (stopSignal.shouldStop()) return false;
  await Promise.race([sleepFn(ms), stopSignal.whenStopped()]);
  return !stopSignal.shouldStop();
}

// --- interrupted review + terminal status ---------------------------------

/**
 * A stop requested mid-review normally finishes the in-flight review (a
 * half-posted review is worse than a slightly late shutdown), unless the
 * head has already moved, in which case the in-flight work is for a dead
 * head and must abort rather than post a stale verdict.
 */
export function interruptedReviewDecision({ stopRequested, headChangedDuringReview }) {
  if (!stopRequested) return { action: "continue" };
  if (headChangedDuringReview) return { action: "abort", reason: "head changed during review" };
  return { action: "finish", reason: "stop requested; finishing in-flight review before terminating" };
}

/** The single terminal status emitted exactly once when the worker actually stops. */
export function terminalStatus({ reason, reviewsCompleted, lastError, timestamp }) {
  return { event: "terminal_status", reason, reviewsCompleted, lastError: lastError ?? null, timestamp };
}

function selfTest() {
  const backoffAfterTwoSteps = stepBackoff(stepBackoff(initialBackoff()));
  assert.equal(backoffAfterTwoSteps.stepIndex, 2);
  assert.equal(resetBackoffOnActivity(backoffAfterTwoSteps).stepIndex, 0);
  assert.equal(backoffDelayMs({ stepIndex: 0 }, { randomFn: () => 0.5 }), 30_000);
  const stop = createStopSignal();
  assert.equal(stop.shouldStop(), false);
  stop.requestStop("test");
  assert.equal(stop.shouldStop(), true);
  stop.requestStop("second call is a no-op");
  assert.equal(stop.reason(), "test");
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 5 })}\n`);
}

if (import.meta.path === Bun.main) {
  if (process.argv.includes("--self-test")) selfTest();
}
