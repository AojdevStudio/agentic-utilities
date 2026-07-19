#!/usr/bin/env bun

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { paneFleetIdentity } from "./fleet-labels.mjs";

const STATUS_VALUES = new Set(["idle", "working", "blocked", "done", "unknown"]);

export function extractPanes(payload) {
  const value = payload?.result?.panes ?? payload?.panes ?? payload?.result ?? payload;
  return Array.isArray(value) ? value : [];
}

export function scopePanes(panes, workspaceId, tabId, projectKey, ownerToken) {
  return panes.filter((pane) => {
    const identity = paneFleetIdentity(pane);
    const ownedWorker =
      identity?.source === "metadata" &&
      identity.key === projectKey &&
      identity.owner === ownerToken &&
      identity.kind === "worker";
    return pane.workspace_id === workspaceId && pane.tab_id === tabId && ownedWorker;
  });
}

export function parseContext(text) {
  const result = {};
  for (const rawLine of text.split(/\r?\n/).toReversed()) {
    const line = rawLine.trim();
    if (!line) continue;

    const bracketed = line.match(/^\[(\d+(?:\.\d+)?)%[^\]]*\]$/i);
    const contextUsed = line.match(/^(\d+(?:\.\d+)?)%\s+context used$/i);
    const tokenFooter = line.match(/^(\d+(?:\.\d+)?)%\s*\/\s*\d+(?:\.\d+)?[km]?$/i);
    const until = line.match(/^(\d+(?:\.\d+)?)%\s+until auto-compact$/i);
    const used = bracketed?.[1] ?? contextUsed?.[1] ?? tokenFooter?.[1];

    if (used !== undefined && result.usedPercent === undefined) result.usedPercent = Number(used);
    if (until !== null && result.untilAutoCompactPercent === undefined) {
      result.untilAutoCompactPercent = Number(until[1]);
    }
    if (used === undefined && until === null) break;
  }
  return result;
}

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? fallback : value;
}

function positiveSeconds(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function positiveInteger(value, fallback) {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function emit(stream, level, event, fields = {}) {
  stream.write(`${JSON.stringify({ level, event, sessionId, generation: instanceToken, ...fields })}\n`);
}

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

function runHerdr(args) {
  const result = spawnSync("herdr", args, {
    encoding: "utf8",
    timeout: 15_000,
    killSignal: "SIGKILL",
  });
  if (result.error || result.status !== 0) {
    const stderr = typeof result.stderr === "string" ? result.stderr.trim() : "";
    throw new Error(stderr || result.error?.message || `herdr ${args.join(" ")} exited ${result.status}`);
  }
  return result.stdout;
}

export function nextCompactionAction(state, options) {
  const { usedPercent, agentStatus, now, retryMs, maxAttempts } = options;
  if (usedPercent !== undefined && usedPercent <= 60 && state.compactionAttempts > 0) {
    state.compactionAttempts = 0;
    state.lastCompactionRequestAt = undefined;
    state.compactionBlocked = false;
    return { type: "rearmed" };
  }
  if (usedPercent === undefined || usedPercent < 75 || state.compactionBlocked) return { type: "none" };
  if (state.compactionAttempts >= maxAttempts) {
    state.compactionBlocked = true;
    return { type: "blocked" };
  }
  if (agentStatus === "blocked") return { type: "deferred" };
  if (state.lastCompactionRequestAt !== undefined && now - state.lastCompactionRequestAt < retryMs) {
    return { type: "none" };
  }
  state.compactionAttempts += 1;
  state.lastCompactionRequestAt = now;
  return { type: "dispatch", attempt: state.compactionAttempts };
}

export function recordCompactionFailure(state, maxAttempts) {
  if (state.compactionAttempts < maxAttempts || state.compactionBlocked) return false;
  state.compactionBlocked = true;
  return true;
}

function selfTest() {
  assert.deepEqual(parseContext("[79% ▮▮]"), { usedPercent: 79 });
  assert.equal(parseContext("90% context used").usedPercent, 90);
  assert.equal(parseContext("9.3%/372k").usedPercent, 9.3);
  assert.equal(parseContext("7% until auto-compact").untilAutoCompactPercent, 7);
  assert.equal(extractPanes({ result: { panes: [{ pane_id: "w1:p1" }] } }).length, 1);
  assert.deepEqual(
    scopePanes(
      [
        { pane_id: "w1:p1", workspace_id: "w1", tab_id: "w1:t1", label: "app-pi-impl" },
        { pane_id: "w1:p2", workspace_id: "w1", tab_id: "w1:t2", label: "app-pi-impl" },
        { pane_id: "w1:p3", workspace_id: "w1", tab_id: "w1:t1", label: "app-1234-pi-impl" },
        {
          pane_id: "w1:p4",
          workspace_id: "w1",
          tab_id: "w1:t1",
          label: "app-custom",
          tokens: { fleet_key: "app", fleet_owner: "owner-1", fleet_kind: "worker" },
        },
      ],
      "w1",
      "w1:t1",
      "app",
      "owner-1",
    ).map((pane) => pane.pane_id),
    ["w1:p4"],
  );
  assert.equal(parseContext("task output: 99% context used\n[79% ▮▮]").usedPercent, 79);
  assert.equal(parseContext("90% context used\n[79% ▮▮]").usedPercent, 79);
  assert.equal(parseContext("12% until auto-compact\n7% until auto-compact").untilAutoCompactPercent, 7);
  assert.equal(parseContext("[79% ▮▮]\nnew prompt output").usedPercent, undefined);
  assert.equal(positiveSeconds("0", 20), 20);
  assert.equal(positiveSeconds("nope", 20), 20);

  const fixture = parseJson(
    readFileSync(new URL("../fixtures/compaction-failures.json", import.meta.url), "utf8"),
    "invalid compaction fixture",
  );
  for (const testCase of fixture.cases) {
    const state = {
      compactionAttempts: 0,
      lastCompactionRequestAt: undefined,
      compactionBlocked: false,
      ...testCase.initialState,
    };
    let blockedEvents = 0;
    testCase.samples.forEach((usedPercent, index) => {
      const action = nextCompactionAction(state, {
        usedPercent,
        agentStatus: testCase.agentStatuses[index],
        now: index * fixture.retryMs,
        retryMs: fixture.retryMs,
        maxAttempts: fixture.maxAttempts,
      });
      if (action.type === "blocked") blockedEvents += 1;
      if (action.type === "dispatch" && testCase.outcomes[index] === "fail") {
        if (recordCompactionFailure(state, fixture.maxAttempts)) blockedEvents += 1;
      }
    });
    assert.equal(state.compactionAttempts, testCase.expectedAttempts, testCase.name);
    assert.equal(blockedEvents, testCase.expectedBlockedEvents, testCase.name);
  }
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 18 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const workspaceId = process.env.HERDR_WORKSPACE_ID;
const tabId = process.env.HERDR_TAB_ID;
const projectKey = option("--project-key");
const ownerToken = option("--owner-token");
const workspaceOption = option("--workspace-id");
const tabOption = option("--tab-id");
const instanceToken = option("--instance-token");
const statusIntervalMs = positiveSeconds(option("--status-interval", "20"), 20) * 1000;
const contextIntervalMs = positiveSeconds(option("--context-interval", "300"), 300) * 1000;
const startupGraceMs = positiveSeconds(option("--startup-grace", "30"), 30) * 1000;
const maxCompactionAttempts = positiveInteger(option("--compaction-attempts", "3"), 3);

if (
  !workspaceId ||
  !tabId ||
  !projectKey ||
  !ownerToken ||
  workspaceOption !== workspaceId ||
  tabOption !== tabId ||
  !instanceToken ||
  !/^[A-Za-z0-9-]+$/.test(instanceToken)
) {
  process.stderr.write(
    `${JSON.stringify({
      level: "fatal",
      event: "invalid_scope",
      sessionId: "fleet:unscoped",
      message: "watch-fleet requires matching workspace/tab arguments plus project, owner, and instance tokens",
    })}\n`,
  );
  process.exit(2);
}

const sessionId = `fleet:${workspaceId}:${tabId}:${projectKey}`;
const states = new Map();
let consecutiveListFailures = 0;
let nextContextPoll = 0;
let stopping = false;

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    stopping = true;
    emit(process.stderr, "info", "watcher_shutdown_requested", { signal });
  });
}

function validateCallerScope() {
  const payload = parseJson(runHerdr(["pane", "current", "--current"]), "invalid current-pane response");
  const pane = payload?.result?.pane ?? payload?.pane ?? payload?.result ?? payload;
  return pane.workspace_id === workspaceId && pane.tab_id === tabId && pane.pane_id === process.env.HERDR_PANE_ID;
}

function listScopedPanes() {
  const payload = parseJson(runHerdr(["pane", "list", "--workspace", workspaceId]), "invalid pane-list response");
  return scopePanes(extractPanes(payload), workspaceId, tabId, projectKey, ownerToken);
}

async function discoverInitialPanes() {
  const deadline = Date.now() + startupGraceMs;
  let message = "no owned worker panes discovered";
  while (!stopping && Date.now() < deadline) {
    try {
      if (!validateCallerScope()) throw new Error("injected workspace, tab, and pane do not match");
      const panes = listScopedPanes();
      if (panes.length > 0) return panes;
    } catch (error) {
      message = error.message;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  emit(process.stderr, "fatal", "invalid_scope", { message });
  process.exitCode = 2;
  return undefined;
}

function emitCompactionBlocked(pane, state) {
  emit(process.stdout, "error", "pane_monitor_blocked", {
    paneId: pane.pane_id,
    reason: "compaction_unconfirmed",
    attempts: state.compactionAttempts,
  });
}

function dispatchCompaction(pane, state, usedPercent, attempt) {
  try {
    runHerdr(["pane", "run", pane.pane_id, "/compact"]);
    emit(process.stdout, "info", "compaction_requested", { paneId: pane.pane_id, usedPercent, attempt });
  } catch (error) {
    emit(process.stderr, "error", "compaction_request_failed", {
      paneId: pane.pane_id,
      attempt,
      message: error.message,
    });
    if (recordCompactionFailure(state, maxCompactionAttempts)) emitCompactionBlocked(pane, state);
  }
}

function pollContext(pane, state) {
  let output;
  try {
    output = runHerdr(["pane", "read", pane.pane_id, "--source", "visible", "--lines", "80"]);
    state.contextFailures = 0;
  } catch (error) {
    state.contextFailures += 1;
    emit(process.stderr, "warn", "context_read_failed", {
      paneId: pane.pane_id,
      failures: state.contextFailures,
      message: error.message,
    });
    if (state.contextFailures >= 3) {
      emit(process.stdout, "error", "pane_monitor_blocked", { paneId: pane.pane_id });
    }
    return;
  }

  const context = parseContext(output);
  const action = nextCompactionAction(state, {
    usedPercent: context.usedPercent,
    agentStatus: pane.agent_status,
    now: Date.now(),
    retryMs: Math.max(contextIntervalMs, 60_000),
    maxAttempts: maxCompactionAttempts,
  });
  if (action.type === "rearmed") {
    emit(process.stdout, "info", "compaction_rearmed", { paneId: pane.pane_id });
  } else if (action.type === "deferred") {
    emit(process.stdout, "warn", "compaction_deferred_blocked_dialog", {
      paneId: pane.pane_id,
      usedPercent: context.usedPercent,
    });
  } else if (action.type === "blocked") {
    emitCompactionBlocked(pane, state);
  } else if (action.type === "dispatch") {
    dispatchCompaction(pane, state, context.usedPercent, action.attempt);
  }

  const lowRemaining = context.untilAutoCompactPercent !== undefined && context.untilAutoCompactPercent <= 8;
  if (lowRemaining && !state.autoCompactWarning) {
    state.autoCompactWarning = true;
    emit(process.stdout, "warn", "auto_compact_near", {
      paneId: pane.pane_id,
      remainingPercent: context.untilAutoCompactPercent,
    });
  } else if (!lowRemaining) {
    state.autoCompactWarning = false;
  }
}

async function main() {
  const initialPanes = await discoverInitialPanes();
  if (!initialPanes) return;

  emit(process.stderr, "info", "watcher_started", {
    workspaceId,
    tabId,
    projectKey,
    ownerToken,
    statusIntervalMs,
    contextIntervalMs,
    startupGraceMs,
    maxCompactionAttempts,
    initialPaneCount: initialPanes.length,
  });

  while (!stopping) {
    let panes;
    try {
      panes = listScopedPanes();
      consecutiveListFailures = 0;
    } catch (error) {
      consecutiveListFailures += 1;
      emit(process.stderr, "error", "pane_list_failed", {
        failures: consecutiveListFailures,
        message: error.message,
      });
      if (consecutiveListFailures >= 3) {
        emit(process.stderr, "fatal", "watcher_stopped_after_failures");
        process.exitCode = 1;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, statusIntervalMs));
      continue;
    }

    const liveIds = new Set(panes.map((pane) => pane.pane_id));
    for (const [paneId] of states) {
      if (!liveIds.has(paneId)) {
        states.delete(paneId);
        emit(process.stdout, "info", "pane_removed", { paneId });
      }
    }

    const contextDue = Date.now() >= nextContextPoll;
    for (const pane of panes) {
      const state = states.get(pane.pane_id) ?? {
        status: undefined,
        compactionAttempts: 0,
        lastCompactionRequestAt: undefined,
        compactionBlocked: false,
        autoCompactWarning: false,
        contextFailures: 0,
      };
      const status = STATUS_VALUES.has(pane.agent_status) ? pane.agent_status : "unknown";
      if (state.status !== status) {
        emit(process.stdout, "info", "pane_status_changed", {
          paneId: pane.pane_id,
          label: pane.label,
          previous: state.status,
          status,
        });
        state.status = status;
      }
      states.set(pane.pane_id, state);
      if (contextDue) pollContext({ ...pane, agent_status: status }, state);
    }

    if (contextDue) nextContextPoll = Date.now() + contextIntervalMs;
    await new Promise((resolve) => setTimeout(resolve, statusIntervalMs));
  }

  emit(process.stderr, "info", "watcher_stopped");
}

await main();
