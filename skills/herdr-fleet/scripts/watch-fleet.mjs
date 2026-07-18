#!/usr/bin/env bun

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";

const STATUS_VALUES = new Set(["idle", "working", "blocked", "done", "unknown"]);

export function extractPanes(payload) {
  const value = payload?.result?.panes ?? payload?.panes ?? payload?.result ?? payload;
  return Array.isArray(value) ? value : [];
}

export function scopePanes(panes, workspaceId, tabId, projectKey) {
  const labelPrefix = `${projectKey}-`;
  return panes.filter(
    (pane) =>
      pane.workspace_id === workspaceId &&
      pane.tab_id === tabId &&
      typeof pane.label === "string" &&
      pane.label.startsWith(labelPrefix) &&
      pane.label !== `${projectKey}-control-pane`,
  );
}

export function parseContext(text) {
  const patterns = [
    /\[(\d+(?:\.\d+)?)%/i,
    /(\d+(?:\.\d+)?)%\s*context used/i,
    /(\d+(?:\.\d+)?)%\s*\/\s*\d+(?:\.\d+)?[km]?/i,
  ];
  const used = patterns.map((pattern) => text.match(pattern)?.[1]).find(Boolean);
  const until = text.match(/(\d+(?:\.\d+)?)%\s*until auto-compact/i)?.[1];
  return {
    usedPercent: used === undefined ? undefined : Number(used),
    untilAutoCompactPercent: until === undefined ? undefined : Number(until),
  };
}

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

function emit(stream, level, event, fields = {}) {
  stream.write(`${JSON.stringify({ level, event, sessionId, ...fields })}\n`);
}

function runHerdr(args) {
  const result = spawnSync("herdr", args, { encoding: "utf8" });
  if (result.error || result.status !== 0) {
    const stderr = typeof result.stderr === "string" ? result.stderr.trim() : "";
    throw new Error(stderr || result.error?.message || `herdr ${args.join(" ")} exited ${result.status}`);
  }
  return result.stdout;
}

function selfTest() {
  assert.deepEqual(parseContext("[79% ▮▮]"), {
    usedPercent: 79,
    untilAutoCompactPercent: undefined,
  });
  assert.equal(parseContext("90% context used").usedPercent, 90);
  assert.equal(parseContext("9.3%/372k").usedPercent, 9.3);
  assert.equal(parseContext("7% until auto-compact").untilAutoCompactPercent, 7);
  assert.equal(extractPanes({ result: { panes: [{ pane_id: "w1:p1" }] } }).length, 1);
  assert.deepEqual(
    scopePanes(
      [
        { pane_id: "w1:p1", workspace_id: "w1", tab_id: "w1:t1", label: "app-pi-impl" },
        { pane_id: "w1:p2", workspace_id: "w1", tab_id: "w1:t2", label: "app-pi-impl" },
      ],
      "w1",
      "w1:t1",
      "app",
    ).map((pane) => pane.pane_id),
    ["w1:p1"],
  );
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 6 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const workspaceId = process.env.HERDR_WORKSPACE_ID;
const tabId = process.env.HERDR_TAB_ID;
const projectKey = option("--project-key");
const statusIntervalMs = Number(option("--status-interval", "20")) * 1000;
const contextIntervalMs = Number(option("--context-interval", "300")) * 1000;

if (!workspaceId || !tabId || !projectKey) {
  process.stderr.write(
    `${JSON.stringify({
      level: "fatal",
      event: "invalid_scope",
      sessionId: "fleet:unscoped",
      message: "watch-fleet requires HERDR_WORKSPACE_ID, HERDR_TAB_ID, and --project-key",
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

function listScopedPanes() {
  const payload = JSON.parse(runHerdr(["pane", "list", "--workspace", workspaceId]));
  return scopePanes(extractPanes(payload), workspaceId, tabId, projectKey);
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
  if (context.usedPercent !== undefined && context.usedPercent <= 60 && state.compactionRequested) {
    state.compactionRequested = false;
    emit(process.stdout, "info", "compaction_rearmed", { paneId: pane.pane_id });
  }

  if (context.usedPercent !== undefined && context.usedPercent >= 75 && !state.compactionRequested) {
    if (pane.agent_status === "blocked") {
      emit(process.stdout, "warn", "compaction_deferred_blocked_dialog", {
        paneId: pane.pane_id,
        usedPercent: context.usedPercent,
      });
    } else {
      try {
        runHerdr(["pane", "run", pane.pane_id, "/compact"]);
        state.compactionRequested = true;
        emit(process.stdout, "info", "compaction_requested", {
          paneId: pane.pane_id,
          usedPercent: context.usedPercent,
        });
      } catch (error) {
        emit(process.stderr, "error", "compaction_request_failed", {
          paneId: pane.pane_id,
          message: error.message,
        });
      }
    }
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
  emit(process.stderr, "info", "watcher_started", {
    workspaceId,
    tabId,
    projectKey,
    statusIntervalMs,
    contextIntervalMs,
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
        compactionRequested: false,
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
