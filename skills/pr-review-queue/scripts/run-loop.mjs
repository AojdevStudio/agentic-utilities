import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import {
  backoffDelayMs,
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

async function pollPr(pr, fetchPrState, observations, now) {
  const { head, election, gateEvidence } = await fetchPrState(pr);
  const previous = observations.get(pr) ?? initialObservation(pr);
  const observation = updateObservation(previous, { head, gateEvidence, election, now });
  const action = nextAction(election, observationChanged(previous, observation));
  return { observation, actionable: action === "skip" ? null : { pr, head, action } };
}

export async function pollOnce({ fetchQueue, fetchPrState, observations, now }) {
  const queue = await fetchQueue();
  const actionable = [];
  const errors = [];
  const nextObservations = new Map(observations);
  for (const pr of queue) {
    try {
      const result = await pollPr(pr, fetchPrState, observations, now);
      nextObservations.set(pr, result.observation);
      if (result.actionable) actionable.push(result.actionable);
    } catch (error) {
      errors.push({ pr, message: error.message });
    }
  }
  if (queue.length > 0 && errors.length === queue.length) {
    throw new Error(`all ${queue.length} queued PRs failed to poll: ${errors[0].message}`);
  }
  return { actionable, observations: nextObservations, errors };
}

export function loadObservations(path) {
  if (!existsSync(path)) return new Map();
  const serialized = readFileSync(path, "utf8");
  try {
    const entries = JSON.parse(serialized);
    return new Map(entries.map((entry) => [entry.pr, entry]));
  } catch (error) {
    if (!(error instanceof SyntaxError)) throw error;
    return new Map();
  }
}

export function saveObservations(path, observations) {
  const tmp = `${path}.tmp`;
  writeFileSync(tmp, `${JSON.stringify([...observations.values()], null, 2)}\n`);
  renameSync(tmp, path);
}

function emitResult(result, emit, now) {
  for (const error of result.errors) emit({ event: "pr_poll_error", ...error, timestamp: now });
  if (result.actionable.length > 0) {
    emit({ event: "actionable_prs", prs: result.actionable, timestamp: now });
    return true;
  }
  return false;
}

async function pollCycle({ fetchQueue, fetchPrState, observations, statePath, backoff, now, emit }) {
  const result = await pollOnce({ fetchQueue, fetchPrState, observations, now });
  saveObservations(statePath, result.observations);
  const active = emitResult(result, emit, now);
  const nextBackoff = active ? resetBackoffOnActivity(backoff) : stepBackoff(backoff);
  if (!active) emit({ event: "heartbeat", queueEmpty: true, stepIndex: nextBackoff.stepIndex, timestamp: now });
  return { observations: result.observations, backoff: nextBackoff };
}

export async function runLoop(options) {
  const { statePath, stopSignal, nowFn, sleepFn, randomFn, maxErrors = 5, emit = () => {} } = options;
  let observations = loadObservations(statePath);
  let backoff = initialBackoff();
  let lastError = null;
  while (!stopSignal.shouldStop()) {
    try {
      ({ observations, backoff } = await pollCycle({ ...options, observations, backoff, now: nowFn(), emit }));
    } catch (error) {
      lastError = error.message;
      backoff = recordError(backoff, maxErrors);
      emit({ event: "poll_error", message: error.message, consecutiveErrors: backoff.consecutiveErrors });
      if (backoff.aborted) break;
    }
    if (stopSignal.shouldStop()) break;
    const delay = backoffDelayMs(backoff, { randomFn });
    if (!(await sleepUnlessStopped(delay, stopSignal, sleepFn))) break;
  }
  const status = terminalStatus({
    reason: stopSignal.reason() ?? (lastError ? `aborted after ${maxErrors} consecutive errors` : "stopped"),
    reviewsCompleted: 0,
    lastError,
    timestamp: nowFn(),
  });
  emit(status);
  return status;
}
