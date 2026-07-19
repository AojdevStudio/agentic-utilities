#!/usr/bin/env bun

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  fstatSync,
  openSync,
  readFileSync,
  readSync,
  renameSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

function cursorAt(cursorPath) {
  if (!existsSync(cursorPath)) return 0;
  const value = Number(readFileSync(cursorPath, "utf8").trim());
  if (!Number.isSafeInteger(value) || value < 0) throw new Error("invalid cursor file");
  return value;
}

export function readFromCursor(eventsPath, cursor, maxBytes = 65_536) {
  const descriptor = openSync(eventsPath, "r");
  try {
    const size = fstatSync(descriptor).size;
    if (cursor > size) throw new Error("event file truncated behind saved cursor");
    const length = Math.min(size - cursor, maxBytes);
    const buffer = Buffer.alloc(length);
    return buffer.subarray(0, readSync(descriptor, buffer, 0, length, cursor));
  } finally {
    closeSync(descriptor);
  }
}

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

export function nextEvent(eventsPath, cursor, expectedSessionId) {
  const data = readFromCursor(eventsPath, cursor);
  const newline = data.indexOf(0x0a);
  if (newline === -1) {
    if (data.length === 65_536) throw new Error("event exceeds maximum line size");
    return undefined;
  }
  const line = data.subarray(0, newline).toString("utf8").trim();
  if (!line) return { event: undefined, nextCursor: cursor + newline + 1 };
  const event = parseJson(line, "invalid watcher event");
  if (event.sessionId !== expectedSessionId) throw new Error("event session scope mismatch");
  return { event, nextCursor: cursor + newline + 1 };
}

export function watcherIdentityMatches(candidate) {
  const { pid, storedToken, expectedCommandSuffix, command } = candidate;
  if (!Number.isSafeInteger(pid) || pid <= 1 || !/^[A-Za-z0-9-]+$/.test(storedToken ?? "")) return false;
  if (!expectedCommandSuffix?.endsWith(`--instance-token ${storedToken}`)) return false;
  return command?.trimEnd().endsWith(expectedCommandSuffix) ?? false;
}

function watcherRunning(pidPath, instanceTokenPath, expectedCommandSuffix) {
  if (!existsSync(pidPath) || !existsSync(instanceTokenPath)) return false;
  const pid = Number(readFileSync(pidPath, "utf8").trim());
  const storedToken = readFileSync(instanceTokenPath, "utf8").trim();
  const result = spawnSync("ps", ["-ww", "-p", String(pid), "-o", "command="], {
    encoding: "utf8",
    timeout: 5000,
  });
  if (result.error || result.status !== 0) return false;
  return watcherIdentityMatches({ pid, storedToken, expectedCommandSuffix, command: result.stdout });
}

function acknowledge(eventsPath, cursorPath, nextCursor) {
  const current = cursorAt(cursorPath);
  const size = statSync(eventsPath).size;
  if (!Number.isSafeInteger(nextCursor) || nextCursor <= current || nextCursor > size) {
    throw new Error("ack cursor is outside the pending event range");
  }
  const boundary = readFromCursor(eventsPath, nextCursor - 1, 1);
  if (boundary[0] !== 0x0a) throw new Error("ack cursor is not an event boundary");
  const temporary = `${cursorPath}.tmp-${process.pid}`;
  writeFileSync(temporary, `${nextCursor}\n`, { mode: 0o600 });
  renameSync(temporary, cursorPath);
}

async function readNext(
  eventsPath,
  cursorPath,
  pidPath,
  instanceTokenPath,
  expectedCommandSuffix,
  expectedSessionId,
  waitMs,
) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    if (!watcherRunning(pidPath, instanceTokenPath, expectedCommandSuffix)) {
      throw new Error("watcher identity changed before the next complete event");
    }
    const cursor = cursorAt(cursorPath);
    if (existsSync(eventsPath)) {
      const result = nextEvent(eventsPath, cursor, expectedSessionId);
      if (result) return result;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return undefined;
}

function selfTest() {
  const root = path.join(tmpdir(), `herdr-fleet-consumer-${process.pid}`);
  const eventsPath = `${root}.ndjson`;
  const cursorPath = `${root}.cursor`;
  const sessionId = "fleet:w1:w1:t1:app";
  const first = `${JSON.stringify({ sessionId, event: "first" })}\n`;
  writeFileSync(eventsPath, `${first}${JSON.stringify({ sessionId, event: "partial" })}`);
  const result = nextEvent(eventsPath, 0, sessionId);
  assert.equal(result.event.event, "first");
  acknowledge(eventsPath, cursorPath, result.nextCursor);
  assert.equal(cursorAt(cursorPath), Buffer.byteLength(first));
  assert.equal(nextEvent(eventsPath, cursorAt(cursorPath), sessionId), undefined);
  assert.throws(() => nextEvent(eventsPath, 0, "fleet:other"));
  const tail = readFromCursor(eventsPath, cursorAt(cursorPath), 9);
  assert.equal(tail.length, 9);
  assert.equal(tail.toString("utf8"), '{"session');

  const identityFixture = parseJson(
    readFileSync(new URL("../fixtures/watcher-identity.json", import.meta.url), "utf8"),
    "invalid watcher identity fixture",
  );
  for (const testCase of identityFixture.cases) {
    const candidate = { ...testCase, storedToken: testCase.instanceId };
    assert.equal(watcherIdentityMatches(candidate), testCase.expected, testCase.name);
  }
  unlinkSync(eventsPath);
  unlinkSync(cursorPath);
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 9 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const eventsPath = option("--events");
const cursorPath = option("--cursor");
const pidPath = option("--pid-file");
const instanceTokenPath = option("--instance-token-file");
const expectedCommandSuffix = option("--watcher-command-suffix");
const expectedSessionId = option("--session-id");
if (!eventsPath || !cursorPath || !expectedSessionId) {
  process.stderr.write(
    `${JSON.stringify({
      level: "fatal",
      event: "invalid_arguments",
      sessionId: expectedSessionId ?? "fleet:unscoped",
    })}\n`,
  );
  process.exit(2);
}

try {
  if (process.argv.includes("--next")) {
    if (!pidPath || !instanceTokenPath || !expectedCommandSuffix) {
      throw new Error("--next requires PID, instance-token, and command-suffix identity");
    }
    const waitSeconds = Number(option("--wait-seconds") ?? "30");
    const waitMs = Number.isFinite(waitSeconds) && waitSeconds > 0 ? waitSeconds * 1000 : 30_000;
    const result = await readNext(
      eventsPath,
      cursorPath,
      pidPath,
      instanceTokenPath,
      expectedCommandSuffix,
      expectedSessionId,
      waitMs,
    );
    process.stdout.write(`${JSON.stringify(result ?? { status: "no_event" })}\n`);
  } else if (process.argv.includes("--ack")) {
    acknowledge(eventsPath, cursorPath, Number(option("--ack")));
    process.stdout.write(`${JSON.stringify({ status: "acknowledged" })}\n`);
  } else {
    throw new Error("choose --next or --ack");
  }
} catch (error) {
  process.stderr.write(
    `${JSON.stringify({
      level: "error",
      event: "consumer_failed",
      sessionId: expectedSessionId,
      message: error.message,
    })}\n`,
  );
  process.exit(1);
}
