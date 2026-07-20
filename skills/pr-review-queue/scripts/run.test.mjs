import { test } from "bun:test";
import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createStopSignal } from "./poll-state.mjs";
import { loadObservations, pollOnce, runLoop, saveObservations } from "./run.mjs";

function tempStatePath() {
  const dir = mkdtempSync(join(tmpdir(), "pr-review-queue-"));
  return join(dir, "state.json");
}

function evidence(overrides = {}) {
  return {
    schemaVersion: 1,
    headSha: "abc123",
    checkedAt: "2026-07-20T00:00:00Z",
    threads: [],
    blockingThreadIds: [],
    humanBlockingThreadIds: [],
    automatedBlockingThreadIds: [],
    checks: [{ name: "lint", state: "pass", required: true, synthesized: false }],
    overall: "pass",
    ...overrides,
  };
}

function election(overrides = {}) {
  return { head: "abc123", claims: [], winner: null, ...overrides };
}

// --- pollOnce: decision logic over an injected queue + PR state ---

test("pollOnce flags a PR with no claim as needing a full review", async () => {
  const { actionable, observations } = await pollOnce({
    fetchQueue: async () => [35],
    fetchPrState: async () => ({ head: "abc123", election: election(), gateEvidence: evidence() }),
    observations: new Map(),
    now: "2026-07-20T00:00:00Z",
  });
  assert.deepEqual(actionable, [{ pr: 35, head: "abc123", action: "full-review" }]);
  assert.ok(observations.has(35));
});

test("pollOnce skips a PR actively (and freshly) claimed by another worker, and stays skipped when nothing changes", async () => {
  const fetchQueue = async () => [35];
  const fetchPrState = async () => ({
    head: "abc123",
    election: election({
      winner: { claimId: "c1", head: "abc123", worker: "other", state: "active", reclaimable: false },
    }),
    gateEvidence: evidence(),
  });
  const first = await pollOnce({ fetchQueue, fetchPrState, observations: new Map(), now: "t1" });
  assert.deepEqual(first.actionable, []);
  const second = await pollOnce({ fetchQueue, fetchPrState, observations: first.observations, now: "t2" });
  assert.deepEqual(second.actionable, [], "unchanged claim/gate state must not re-flag on the next poll");
});

test("pollOnce requests gate-only revalidation exactly when gate evidence changes for a completed claim", async () => {
  const fetchQueue = async () => [35];
  let state = "pass";
  const fetchPrState = async () => ({
    head: "abc123",
    election: election({ winner: { claimId: "c1", head: "abc123", worker: "w1", state: "completed" } }),
    gateEvidence: evidence({ overall: state, checks: [{ name: "lint", state, required: true, synthesized: false }] }),
  });

  const first = await pollOnce({ fetchQueue, fetchPrState, observations: new Map(), now: "t1" });
  assert.equal(
    first.actionable[0]?.action,
    "gate-only",
    "an unobserved completed claim must be gate-verified at least once",
  );

  const second = await pollOnce({ fetchQueue, fetchPrState, observations: first.observations, now: "t2" });
  assert.deepEqual(second.actionable, [], "unchanged gate evidence on a completed claim must not re-flag");

  state = "fail";
  const third = await pollOnce({ fetchQueue, fetchPrState, observations: second.observations, now: "t3" });
  assert.equal(
    third.actionable[0]?.action,
    "gate-only",
    "a changed check state must re-trigger gate-only revalidation even though the claim itself didn't change",
  );
});

// --- persisted state on disk ---

test("saveObservations/loadObservations round-trip through disk", () => {
  const path = tempStatePath();
  const observations = new Map([
    [35, { pr: 35, head: "abc123", updatedAt: "t1", gateFingerprint: "f1", claimState: null }],
  ]);
  saveObservations(path, observations);
  const loaded = loadObservations(path);
  assert.deepEqual(loaded.get(35), observations.get(35));
});

test("loadObservations starts empty when no state file exists yet", () => {
  const loaded = loadObservations(tempStatePath());
  assert.equal(loaded.size, 0);
});

// --- runLoop: the executable persisted loop itself ---

test("runLoop never terminates on an empty queue; it heartbeats with backoff until stopped", async () => {
  const stopSignal = createStopSignal();
  const events = [];
  let cycles = 0;
  const status = await runLoop({
    fetchQueue: async () => {
      cycles += 1;
      if (cycles >= 3) stopSignal.requestStop("control pane stop");
      return [];
    },
    fetchPrState: async () => {
      throw new Error("must not be called for an empty queue");
    },
    statePath: tempStatePath(),
    stopSignal,
    nowFn: () => "2026-07-20T00:00:00Z",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: (event) => events.push(event),
  });
  const heartbeats = events.filter((event) => event.event === "heartbeat");
  assert.equal(heartbeats.length, 3, "queue-empty must heartbeat every cycle, not terminate early");
  assert.deepEqual(
    heartbeats.map((h) => h.stepIndex),
    [1, 2, 3],
  );
  const terminal = events.filter((event) => event.event === "terminal_status");
  assert.equal(terminal.length, 1, "exactly one terminal status must be emitted");
  assert.equal(status.event, "terminal_status");
  assert.equal(status.reason, "control pane stop");
});

test("runLoop resets backoff to the shortest interval once actionable work appears", async () => {
  const stopSignal = createStopSignal();
  const events = [];
  let cycle = 0;
  const status = await runLoop({
    fetchQueue: async () => {
      cycle += 1;
      if (cycle === 3) return [35];
      if (cycle >= 4) stopSignal.requestStop("done");
      return [];
    },
    fetchPrState: async () => ({ head: "abc123", election: election(), gateEvidence: evidence() }),
    statePath: tempStatePath(),
    stopSignal,
    nowFn: () => "t",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: (event) => events.push(event),
  });
  const heartbeats = events.filter((event) => event.event === "heartbeat").map((h) => h.stepIndex);
  const activity = events.filter((event) => event.event === "actionable_prs");
  assert.deepEqual(
    heartbeats,
    [1, 2, 1],
    "the post-activity idle cycle must resume from the reset step, not the pre-activity step",
  );
  assert.equal(activity.length, 1);
  assert.equal(activity[0].prs[0].pr, 35);
  assert.equal(status.reviewsCompleted, 1, "terminal status must report how much actionable work was dispatched");
});

test("runLoop aborts after maxErrors consecutive poll failures and still emits exactly one terminal status", async () => {
  const stopSignal = createStopSignal();
  const events = [];
  const status = await runLoop({
    fetchQueue: async () => {
      throw new Error("network blip");
    },
    fetchPrState: async () => {
      throw new Error("unreachable");
    },
    statePath: tempStatePath(),
    stopSignal,
    nowFn: () => "t",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    maxErrors: 3,
    emit: (event) => events.push(event),
  });
  const errors = events.filter((event) => event.event === "poll_error");
  assert.equal(errors.length, 3, "must abort exactly at maxErrors consecutive failures, not before or after");
  const terminal = events.filter((event) => event.event === "terminal_status");
  assert.equal(terminal.length, 1);
  assert.equal(status.lastError, "network blip");
  assert.ok(status.reason.includes("aborted after 3 consecutive errors"));
});

test("runLoop persists state across a restart: a completed claim with unchanged gates is not re-flagged next run", async () => {
  const statePath = tempStatePath();
  const fetchQueue = async () => [35];
  const fetchPrState = async () => ({
    head: "abc123",
    election: election({ winner: { claimId: "c1", head: "abc123", worker: "w1", state: "completed" } }),
    gateEvidence: evidence(),
  });

  const stop1 = createStopSignal();
  let cycles1 = 0;
  const events1 = [];
  await runLoop({
    fetchQueue: async () => {
      cycles1 += 1;
      if (cycles1 >= 1) stop1.requestStop("cycle done");
      return fetchQueue();
    },
    fetchPrState,
    statePath,
    stopSignal: stop1,
    nowFn: () => "t1",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: (event) => events1.push(event),
  });
  assert.ok(
    events1.some((event) => event.event === "actionable_prs"),
    "first-ever sighting of a completed claim must be gate-verified once",
  );

  const stop2 = createStopSignal();
  let cycles2 = 0;
  const events2 = [];
  await runLoop({
    fetchQueue: async () => {
      cycles2 += 1;
      if (cycles2 >= 1) stop2.requestStop("cycle done");
      return fetchQueue();
    },
    fetchPrState,
    statePath,
    stopSignal: stop2,
    nowFn: () => "t2",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: (event) => events2.push(event),
  });
  assert.ok(
    !events2.some((event) => event.event === "actionable_prs"),
    "a restarted loop must resume from the persisted observation, not re-flag unchanged work",
  );
});

test("runLoop's sleep between cycles is interruptible mid-sleep, not just checked before/after a fixed timer", async () => {
  const stopSignal = createStopSignal();
  let sleepStarted = false;
  let sleepCompletedFully = false;
  const longSleep = (ms) =>
    new Promise((resolve) => {
      sleepStarted = true;
      setTimeout(() => {
        sleepCompletedFully = true;
        resolve();
      }, ms);
    });
  const promise = runLoop({
    fetchQueue: async () => [],
    fetchPrState: async () => {
      throw new Error("unreachable");
    },
    statePath: tempStatePath(),
    stopSignal,
    nowFn: () => "t",
    sleepFn: longSleep,
    randomFn: () => 0.5,
    emit: () => {},
  });
  while (!sleepStarted) await new Promise((resolve) => setTimeout(resolve, 1));
  stopSignal.requestStop("interrupt");
  await promise;
  assert.equal(sleepCompletedFully, false, "must not wait out the full backoff sleep once stopped");
});

test("runLoop never requests a sleep beyond the documented 5-minute backoff cap, even under maximum positive jitter", async () => {
  const stopSignal = createStopSignal();
  const requestedDelays = [];
  let cycle = 0;
  await runLoop({
    fetchQueue: async () => {
      cycle += 1;
      if (cycle > 6) stopSignal.requestStop("done");
      return [];
    },
    fetchPrState: async () => {
      throw new Error("unreachable");
    },
    statePath: tempStatePath(),
    stopSignal,
    nowFn: () => "t",
    sleepFn: async (ms) => {
      requestedDelays.push(ms);
    },
    randomFn: () => 1, // maximum positive jitter every cycle
    emit: () => {},
  });
  assert.ok(requestedDelays.length > 0);
  assert.ok(
    requestedDelays.every((ms) => ms <= 300_000),
    `all requested delays must be <= 300000ms, got ${requestedDelays}`,
  );
});

// --- CLI guard rails ---

const scriptPath = fileURLToPath(new URL("./run.mjs", import.meta.url));

test("run CLI requires --repo and --authorized", () => {
  const missingRepo = Bun.spawnSync(["bun", scriptPath, "--authorized", "alice"]);
  assert.notEqual(missingRepo.exitCode, 0);
  const missingAuth = Bun.spawnSync(["bun", scriptPath, "--repo", "owner/name"]);
  assert.notEqual(missingAuth.exitCode, 0);
});
