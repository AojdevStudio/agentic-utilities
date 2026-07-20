#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import {
  backoffDelayMs,
  createStopSignal,
  initialBackoff,
  initialObservation,
  nextAction,
  observationChanged,
  recordError,
  resetBackoffOnActivity,
  sleepUnlessStopped,
  stepBackoff,
  terminalStatus,
  updateObservation,
} from "./poll-state.mjs";

// The fleet-mode reviewer lane's persisted, interruptible polling loop.
// QUEUE EMPTY must never terminate this loop: it backs off, heartbeats,
// and keeps rechecking open PR heads and gate evidence until the control
// pane requests a stop. All GitHub access is injected (fetchQueue,
// fetchPrState) so the decision logic is fully unit-testable without a
// network; the CLI entrypoint at the bottom wires real `gh`-backed
// fetchers by delegating to queue.mjs/claim.mjs/review-gate.mjs's own
// already-paginated, already-authorized CLIs instead of re-implementing
// GraphQL pagination a fourth time.

/**
 * One poll cycle: walk the reviewable queue, fold fresh claim/gate state
 * into each PR's persisted observation, and report which PRs need action.
 */
export async function pollOnce({ fetchQueue, fetchPrState, observations, now }) {
  const queue = await fetchQueue();
  const actionable = [];
  const nextObservations = new Map(observations);
  for (const pr of queue) {
    const { head, election, gateEvidence: evidence } = await fetchPrState(pr);
    const previous = observations.get(pr) ?? initialObservation(pr);
    const next = updateObservation(previous, { head, gateEvidence: evidence, election, now });
    const gatesChanged = observationChanged(previous, next);
    nextObservations.set(pr, next);
    const action = nextAction(election, gatesChanged);
    if (action !== "skip") actionable.push({ pr, head, action });
  }
  return { actionable, observations: nextObservations };
}

/** Load persisted per-PR observations from disk, or start fresh if the file doesn't exist yet. */
export function loadObservations(path) {
  if (!existsSync(path)) return new Map();
  const entries = JSON.parse(readFileSync(path, "utf8"));
  return new Map(entries.map((entry) => [entry.pr, entry]));
}

/** Persist per-PR observations to disk so a restart or worker handoff resumes from the same state. */
export function saveObservations(path, observations) {
  writeFileSync(path, `${JSON.stringify([...observations.values()], null, 2)}\n`);
}

/**
 * The executable persisted loop: poll, heartbeat or report actionable
 * work, back off with jitter, and stop cleanly and exactly once when the
 * stop signal fires (interruptible mid-sleep, never after a full fixed
 * wait). Repeated poll errors abort after maxErrors consecutive failures.
 */
export async function runLoop({
  fetchQueue,
  fetchPrState,
  statePath,
  stopSignal,
  nowFn,
  sleepFn,
  randomFn,
  maxErrors = 5,
  emit = () => {},
}) {
  let observations = loadObservations(statePath);
  let backoff = initialBackoff();
  let lastError = null;
  let actionableDispatched = 0;

  while (!stopSignal.shouldStop()) {
    try {
      const now = nowFn();
      const result = await pollOnce({ fetchQueue, fetchPrState, observations, now });
      observations = result.observations;
      saveObservations(statePath, observations);
      if (result.actionable.length > 0) {
        actionableDispatched += result.actionable.length;
        backoff = resetBackoffOnActivity(backoff);
        emit({ event: "actionable_prs", prs: result.actionable, timestamp: now });
      } else {
        backoff = stepBackoff(backoff);
        emit({ event: "heartbeat", queueEmpty: true, stepIndex: backoff.stepIndex, timestamp: now });
      }
    } catch (error) {
      lastError = error.message;
      backoff = recordError(backoff, maxErrors);
      emit({ event: "poll_error", message: error.message, consecutiveErrors: backoff.consecutiveErrors });
      if (backoff.aborted) break;
    }

    if (stopSignal.shouldStop()) break;
    const delay = backoffDelayMs(backoff, { randomFn });
    const keepGoing = await sleepUnlessStopped(delay, stopSignal, sleepFn);
    if (!keepGoing) break;
  }

  const status = terminalStatus({
    reason: stopSignal.reason() ?? (lastError ? `aborted after ${maxErrors} consecutive errors` : "stopped"),
    reviewsCompleted: actionableDispatched,
    lastError,
    timestamp: nowFn(),
  });
  emit(status);
  return status;
}

// --- CLI: real gh/bun-backed fetchers, composed from the sibling scripts --

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? fallback : value;
}

function runScript(scriptName, args) {
  const scriptPath = new URL(`./${scriptName}`, import.meta.url).pathname;
  const result = spawnSync("bun", [scriptPath, ...args], { encoding: "utf8", timeout: 30_000 });
  if (result.error) throw new Error(result.error.message);
  try {
    return JSON.parse(result.stdout);
  } catch {
    throw new Error(`${scriptName} produced non-JSON output: ${(result.stderr || result.stdout || "").trim()}`);
  }
}

function currentHead(owner, name, number) {
  const result = spawnSync(
    "gh",
    ["pr", "view", String(number), "-R", `${owner}/${name}`, "--json", "headRefOid", "-q", ".headRefOid"],
    { encoding: "utf8", timeout: 15_000 },
  );
  if (result.error || result.status !== 0) {
    throw new Error(result.stderr?.trim() || result.error?.message || "gh pr view failed");
  }
  return result.stdout.trim();
}

function githubFetchQueue(repository) {
  return async () => runScript("queue.mjs", ["--repo", repository]).queue;
}

function githubFetchPrState(repository, authorizedIdentities, staleMs) {
  const [owner, name] = repository.split("/");
  return async (pr) => {
    const head = currentHead(owner, name, pr);
    const election = runScript("claim.mjs", [
      "--repo",
      repository,
      "--pr",
      String(pr),
      "--expected-head",
      head,
      "--stale-ms",
      String(staleMs),
      "--authorized",
      authorizedIdentities.join(","),
    ]);
    const gateEvidence = runScript("review-gate.mjs", [
      "--repo",
      repository,
      "--pr",
      String(pr),
      "--expected-head",
      head,
    ]);
    return { head, election, gateEvidence };
  };
}

async function main() {
  const repository = option("--repo");
  const statePath = option("--state", ".pr-review-queue-state.json");
  const authorizedIdentities = (option("--authorized") ?? "").split(",").filter(Boolean);
  const staleMs = Number(option("--stale-ms") ?? 20 * 60 * 1000);
  const maxErrors = Number(option("--max-errors") ?? 5);
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra) throw new Error("run requires --repo owner/name");
  if (authorizedIdentities.length === 0) throw new Error("run requires --authorized as a comma-separated list");

  const stopSignal = createStopSignal();
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, () => stopSignal.requestStop(signal));
  }

  const status = await runLoop({
    fetchQueue: githubFetchQueue(repository),
    fetchPrState: githubFetchPrState(repository, authorizedIdentities, staleMs),
    statePath,
    stopSignal,
    nowFn: () => new Date().toISOString(),
    sleepFn: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    randomFn: Math.random,
    maxErrors,
    emit: (event) => process.stdout.write(`${JSON.stringify(event)}\n`),
  });
  process.exitCode = status.lastError ? 1 : 0;
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ level: "fatal", event: "run_failed", message: error.message })}\n`);
    process.exit(1);
  }
}
