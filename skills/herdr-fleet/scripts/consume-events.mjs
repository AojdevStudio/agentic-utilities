#!/usr/bin/env bun

import assert from "node:assert/strict";
import { existsSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs";
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

export function nextEvent(eventsPath, cursor, expectedSessionId) {
  const data = readFileSync(eventsPath);
  if (cursor > data.length) throw new Error("event file truncated behind saved cursor");
  const newline = data.indexOf(0x0a, cursor);
  if (newline === -1) return undefined;
  const line = data.subarray(cursor, newline).toString("utf8").trim();
  if (!line) return { event: undefined, nextCursor: newline + 1 };
  const event = JSON.parse(line);
  if (event.sessionId !== expectedSessionId) throw new Error("event session scope mismatch");
  return { event, nextCursor: newline + 1 };
}

function watcherRunning(pidPath) {
  if (!existsSync(pidPath)) return false;
  const pid = Number(readFileSync(pidPath, "utf8").trim());
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function acknowledge(eventsPath, cursorPath, nextCursor) {
  const current = cursorAt(cursorPath);
  const size = statSync(eventsPath).size;
  if (!Number.isSafeInteger(nextCursor) || nextCursor <= current || nextCursor > size) {
    throw new Error("ack cursor is outside the pending event range");
  }
  const data = readFileSync(eventsPath);
  if (data[nextCursor - 1] !== 0x0a) throw new Error("ack cursor is not an event boundary");
  const temporary = `${cursorPath}.tmp-${process.pid}`;
  writeFileSync(temporary, `${nextCursor}\n`, { mode: 0o600 });
  renameSync(temporary, cursorPath);
}

async function readNext(eventsPath, cursorPath, pidPath, expectedSessionId, waitMs) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    const cursor = cursorAt(cursorPath);
    if (existsSync(eventsPath)) {
      const result = nextEvent(eventsPath, cursor, expectedSessionId);
      if (result) return result;
    }
    if (!watcherRunning(pidPath)) throw new Error("watcher exited before the next complete event");
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
  unlinkSync(eventsPath);
  unlinkSync(cursorPath);
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 4 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const eventsPath = option("--events");
const cursorPath = option("--cursor");
const pidPath = option("--pid-file");
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
    if (!pidPath) throw new Error("--next requires --pid-file");
    const waitSeconds = Number(option("--wait-seconds") ?? "30");
    const waitMs = Number.isFinite(waitSeconds) && waitSeconds > 0 ? waitSeconds * 1000 : 30_000;
    const result = await readNext(eventsPath, cursorPath, pidPath, expectedSessionId, waitMs);
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
