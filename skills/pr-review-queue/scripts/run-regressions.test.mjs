import { test } from "bun:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createStopSignal } from "./poll-state.mjs";
import { loadObservations, pollOnce, runLoop, runScript, saveObservations } from "./run.mjs";

const tempPath = () => join(mkdtempSync(join(tmpdir(), "pr-review-queue-regression-")), "state.json");
const evidence = () => ({
  schemaVersion: 1,
  headSha: "abc123",
  checkedAt: "now",
  threads: [],
  blockingThreadIds: [],
  humanBlockingThreadIds: [],
  automatedBlockingThreadIds: [],
  checks: [{ name: "lint", state: "pass", required: true, synthesized: false }],
  overall: "pass",
});
const election = () => ({ head: "abc123", claims: [], winner: null });

test("pollOnce isolates failures from all per-PR processing", async () => {
  const result = await pollOnce({
    fetchQueue: async () => [35, 36],
    fetchPrState: async (pr) => (pr === 35 ? null : { head: "abc123", election: election(), gateEvidence: evidence() }),
    observations: new Map(),
    now: "now",
  });
  assert.deepEqual(result.actionable, [{ pr: 36, head: "abc123", action: "full-review" }]);
  assert.equal(result.errors[0].pr, 35);
});

test("pollOnce prunes observations for PRs no longer in the queue", async () => {
  const result = await pollOnce({
    fetchQueue: async () => [35],
    fetchPrState: async () => ({ head: "abc123", election: election(), gateEvidence: evidence() }),
    observations: new Map([
      [34, { pr: 34 }],
      [35, { pr: 35 }],
    ]),
    now: "now",
  });
  assert.equal(result.observations.has(34), false);
  assert.equal(result.observations.has(35), true);
});

test("runLoop emits partial PR errors without counting dispatches as completed reviews", async () => {
  const stopSignal = createStopSignal();
  const events = [];
  const status = await runLoop({
    fetchQueue: async () => {
      stopSignal.requestStop("one cycle");
      return [35, 36];
    },
    fetchPrState: async (pr) => {
      if (pr === 35) throw new Error("closed mid-cycle");
      return { head: "abc123", election: election(), gateEvidence: evidence() };
    },
    statePath: tempPath(),
    stopSignal,
    nowFn: () => "now",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: (event) => events.push(event),
  });
  assert.deepEqual(
    events.find((event) => event.event === "pr_poll_error"),
    {
      event: "pr_poll_error",
      pr: 35,
      message: "closed mid-cycle",
      timestamp: "now",
    },
  );
  assert.equal(status.reviewsCompleted, 0);
});

test("loadObservations propagates filesystem errors", () => {
  const directory = mkdtempSync(join(tmpdir(), "pr-review-queue-directory-"));
  assert.throws(() => loadObservations(directory));
});

test("loadObservations recovers from structurally malformed JSON", () => {
  const path = tempPath();
  writeFileSync(path, "{}");
  assert.equal(loadObservations(path).size, 0);
});

test("runLoop clears a transient lastError after a successful cycle", async () => {
  const stopSignal = createStopSignal();
  let cycle = 0;
  const status = await runLoop({
    fetchQueue: async () => {
      cycle += 1;
      if (cycle === 1) throw new Error("transient");
      stopSignal.requestStop("recovered");
      return [];
    },
    fetchPrState: async () => {
      throw new Error("unreachable");
    },
    statePath: tempPath(),
    stopSignal,
    nowFn: () => "now",
    sleepFn: async () => {},
    randomFn: () => 0.5,
    emit: () => {},
  });
  assert.equal(status.lastError, null);
  assert.equal(status.reason, "recovered");
});

test("saveObservations atomically replaces an existing state file", () => {
  const path = tempPath();
  writeFileSync(path, "old state");
  saveObservations(path, new Map([[35, { pr: 35 }]]));
  assert.equal(loadObservations(path).get(35).pr, 35);
  assert.equal(existsSync(`${path}.tmp`), false);
});

test("runScript preserves valid gate evidence returned with exit 3", () => {
  const fixture = fileURLToPath(new URL("../fixtures/gate-evidence.json", import.meta.url));
  const result = runScript(
    "review-gate.mjs",
    ["--repo", "fixture/repository", "--pr", "1", "--expected-head", "abc123", "--fixture", fixture],
    [0, 3],
  );
  assert.equal(result.overall, "fail");
});

test("runScript rejects a nonzero child before parsing its JSON error", () => {
  assert.throws(
    () => runScript("claim.mjs", ["--repo", "fixture/repository", "--pr", "1", "--expected-head", "abc123"]),
    /claim\.mjs exited 1/,
  );
});

test("run CLI rejects invalid numeric bounds before polling", () => {
  const script = fileURLToPath(new URL("./run.mjs", import.meta.url));
  for (const args of [
    ["--max-errors", "nope"],
    ["--max-errors", "0"],
    ["--stale-ms", "-1"],
  ]) {
    const result = Bun.spawnSync(["bun", script, "--repo", "owner/name", "--authorized", "alice", ...args]);
    assert.notEqual(result.exitCode, 0);
    assert.match(new TextDecoder().decode(result.stderr), /positive integer|non-negative number/);
  }
});

test("sleepWithStop keeps an idle subprocess alive until its timer resolves", () => {
  const moduleUrl = new URL("./run.mjs", import.meta.url).href;
  const code = `import { sleepWithStop } from ${JSON.stringify(moduleUrl)}; import { createStopSignal } from ${JSON.stringify(new URL("./poll-state.mjs", import.meta.url).href)}; await sleepWithStop(25, createStopSignal()); process.stdout.write("awake");`;
  const result = Bun.spawnSync(["bun", "-e", code]);
  assert.equal(result.exitCode, 0);
  assert.equal(new TextDecoder().decode(result.stdout), "awake");
});
