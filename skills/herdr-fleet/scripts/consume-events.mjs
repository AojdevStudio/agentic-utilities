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

export function nextEvent(eventsPath, cursor, expectedSessionId, expectedGeneration) {
  const data = readFromCursor(eventsPath, cursor);
  const newline = data.indexOf(0x0a);
  if (newline === -1) {
    if (data.length === 65_536) throw new Error("event exceeds maximum line size");
    return undefined;
  }
  const line = data.subarray(0, newline).toString("utf8").trim();
  if (!line) return { status: "skip", nextCursor: cursor + newline + 1 };
  const event = parseJson(line, "invalid watcher event");
  if (event.sessionId !== expectedSessionId) throw new Error("event session scope mismatch");
  if (event.generation !== expectedGeneration) throw new Error("event watcher generation mismatch");
  return { event, nextCursor: cursor + newline + 1 };
}

export function watcherIdentityMatches(candidate) {
  const { pid, storedToken, requestedToken, expectedCommandSuffix, command } = candidate;
  if (!Number.isSafeInteger(pid) || pid <= 1 || !/^[A-Za-z0-9-]+$/.test(storedToken ?? "")) return false;
  if (requestedToken !== storedToken) return false;
  if (!expectedCommandSuffix?.endsWith(`--instance-token ${storedToken}`)) return false;
  return command?.trimEnd().endsWith(expectedCommandSuffix) ?? false;
}

function watcherRunning(pidPath, instanceTokenPath, expectedCommandSuffix, expectedGeneration) {
  if (!existsSync(pidPath) || !existsSync(instanceTokenPath)) return false;
  const pid = Number(readFileSync(pidPath, "utf8").trim());
  const storedToken = readFileSync(instanceTokenPath, "utf8").trim();
  const result = spawnSync("ps", ["-ww", "-p", String(pid), "-o", "command="], {
    encoding: "utf8",
    timeout: 5000,
  });
  if (result.error || result.status !== 0) return false;
  return watcherIdentityMatches({
    pid,
    storedToken,
    requestedToken: expectedGeneration,
    expectedCommandSuffix,
    command: result.stdout,
  });
}

function startLockManager(lockPath, fileSystem = { existsSync, readFileSync, unlinkSync }) {
  return {
    acquire() {
      if (!lockPath) return false;
      const result = spawnSync("shlock", ["-f", lockPath, "-p", String(process.pid)], { timeout: 5000 });
      return !result.error && result.status === 0;
    },
    release() {
      try {
        if (!fileSystem.existsSync(lockPath)) return;
        if (fileSystem.readFileSync(lockPath, "utf8").trim() === String(process.pid)) {
          fileSystem.unlinkSync(lockPath);
        }
      } catch (error) {
        if (error?.code !== "ENOENT") throw error;
      }
    },
  };
}

function acknowledgeLocked(options) {
  const {
    eventsPath,
    cursorPath,
    nextCursor,
    expectedSessionId,
    expectedGeneration,
    pidPath,
    instanceTokenPath,
    expectedCommandSuffix,
    identityValidator,
  } = options;
  if (!identityValidator(pidPath, instanceTokenPath, expectedCommandSuffix, expectedGeneration)) {
    throw new Error("watcher identity changed before acknowledgment");
  }
  const current = cursorAt(cursorPath);
  const size = statSync(eventsPath).size;
  if (!Number.isSafeInteger(nextCursor) || nextCursor <= current || nextCursor > size) {
    throw new Error("ack cursor is outside the pending event range");
  }
  const pending = nextEvent(eventsPath, current, expectedSessionId, expectedGeneration);
  if (!pending || pending.nextCursor !== nextCursor) throw new Error("ack does not match pending event");
  const temporary = `${cursorPath}.tmp-${process.pid}`;
  writeFileSync(temporary, `${nextCursor}\n`, { mode: 0o600 });
  if (!identityValidator(pidPath, instanceTokenPath, expectedCommandSuffix, expectedGeneration)) {
    unlinkSync(temporary);
    throw new Error("watcher identity changed during acknowledgment");
  }
  renameSync(temporary, cursorPath);
}

export function acknowledge(options) {
  const lockManager = options.lockManager ?? startLockManager(options.startLockPath);
  if (!lockManager.acquire()) throw new Error("watcher start lock unavailable during acknowledgment");
  try {
    acknowledgeLocked({ ...options, identityValidator: options.identityValidator ?? watcherRunning });
  } finally {
    lockManager.release();
  }
}

async function readNext(
  eventsPath,
  cursorPath,
  pidPath,
  instanceTokenPath,
  expectedCommandSuffix,
  expectedSessionId,
  expectedGeneration,
  waitMs,
) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    if (!watcherRunning(pidPath, instanceTokenPath, expectedCommandSuffix, expectedGeneration)) {
      throw new Error("watcher identity changed before the next complete event");
    }
    const cursor = cursorAt(cursorPath);
    if (existsSync(eventsPath)) {
      const result = nextEvent(eventsPath, cursor, expectedSessionId, expectedGeneration);
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
  const generation = "instance-a";
  const first = `${JSON.stringify({ sessionId, generation, event: "first" })}\n`;
  writeFileSync(eventsPath, `${first}${JSON.stringify({ sessionId, generation, event: "partial" })}`);
  const result = nextEvent(eventsPath, 0, sessionId, generation);
  assert.equal(result.event.event, "first");
  const ackOptions = {
    eventsPath,
    cursorPath,
    nextCursor: result.nextCursor,
    expectedSessionId: sessionId,
    expectedGeneration: generation,
    pidPath: `${root}.pid`,
    instanceTokenPath: `${root}.instance-token`,
    expectedCommandSuffix: "watch-fleet.mjs --instance-token instance-a",
    lockManager: { acquire: () => true, release: () => {} },
  };
  const replacementIdentity = watcherIdentityMatches({
    pid: 4242,
    storedToken: "instance-b",
    requestedToken: generation,
    expectedCommandSuffix: "watch-fleet.mjs --instance-token instance-b",
    command: "bun watch-fleet.mjs --instance-token instance-b",
  });
  assert.throws(() => acknowledge({ ...ackOptions, identityValidator: () => replacementIdentity }), /identity changed/);
  assert.equal(cursorAt(cursorPath), 0);
  const missingLockError = Object.assign(new Error("lock disappeared"), { code: "ENOENT" });
  let racedLockRead = false;
  const racedLock = startLockManager(`${root}.race-lock`, {
    existsSync: () => true,
    readFileSync: () => {
      racedLockRead = true;
      throw missingLockError;
    },
    unlinkSync: () => assert.fail("missing lock must not be unlinked"),
  });
  assert.throws(
    () =>
      acknowledge({
        ...ackOptions,
        identityValidator: () => false,
        lockManager: { acquire: () => true, release: () => racedLock.release() },
      }),
    /watcher identity changed/,
  );
  assert.equal(racedLockRead, true);
  let identityChecks = 0;
  assert.throws(
    () =>
      acknowledge({
        ...ackOptions,
        identityValidator: () => {
          identityChecks += 1;
          return identityChecks === 1;
        },
      }),
    /identity changed during acknowledgment/,
  );
  assert.equal(cursorAt(cursorPath), 0);
  assert.throws(
    () =>
      acknowledge({
        ...ackOptions,
        expectedSessionId: "fleet:other",
        identityValidator: () => true,
      }),
    /session scope mismatch/,
  );
  assert.equal(cursorAt(cursorPath), 0);
  assert.throws(
    () =>
      acknowledge({
        ...ackOptions,
        expectedGeneration: "instance-b",
        identityValidator: () => true,
      }),
    /generation mismatch/,
  );
  assert.equal(cursorAt(cursorPath), 0);
  assert.throws(
    () =>
      acknowledge({
        ...ackOptions,
        nextCursor: result.nextCursor + 1,
        identityValidator: () => true,
      }),
    /pending event/,
  );
  assert.equal(cursorAt(cursorPath), 0);
  const lockCalls = [];
  acknowledge({
    ...ackOptions,
    identityValidator: () => {
      lockCalls.push("identity");
      return true;
    },
    lockManager: {
      acquire: () => {
        lockCalls.push("acquire");
        return true;
      },
      release: () => {
        lockCalls.push("release");
        assert.equal(cursorAt(cursorPath), result.nextCursor);
      },
    },
  });
  assert.deepEqual(lockCalls, ["acquire", "identity", "identity", "release"]);
  assert.equal(cursorAt(cursorPath), Buffer.byteLength(first));
  assert.equal(nextEvent(eventsPath, cursorAt(cursorPath), sessionId, generation), undefined);
  assert.throws(() => nextEvent(eventsPath, 0, "fleet:other", generation));
  assert.throws(() => nextEvent(eventsPath, 0, sessionId, "instance-b"));
  const tail = readFromCursor(eventsPath, cursorAt(cursorPath), 9);
  assert.equal(tail.length, 9);
  assert.equal(tail.toString("utf8"), '{"session');

  const blankEventsPath = `${root}.blank.ndjson`;
  const blankCursorPath = `${root}.blank.cursor`;
  writeFileSync(blankEventsPath, "\n");
  const blank = nextEvent(blankEventsPath, 0, sessionId, generation);
  assert.equal(blank.status, "skip");
  assert.equal(blank.nextCursor, 1);
  acknowledge({
    ...ackOptions,
    eventsPath: blankEventsPath,
    cursorPath: blankCursorPath,
    nextCursor: blank.nextCursor,
    identityValidator: () => true,
  });
  assert.equal(cursorAt(blankCursorPath), 1);

  const identityFixture = parseJson(
    readFileSync(new URL("../fixtures/watcher-identity.json", import.meta.url), "utf8"),
    "invalid watcher identity fixture",
  );
  for (const testCase of identityFixture.cases) {
    const candidate = {
      ...testCase,
      storedToken: testCase.instanceId,
      requestedToken: testCase.instanceId,
    };
    assert.equal(watcherIdentityMatches(candidate), testCase.expected, testCase.name);
  }
  unlinkSync(eventsPath);
  unlinkSync(cursorPath);
  unlinkSync(blankEventsPath);
  unlinkSync(blankCursorPath);
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 27 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const eventsPath = option("--events");
const cursorPath = option("--cursor");
const pidPath = option("--pid-file");
const instanceTokenPath = option("--instance-token-file");
const startLockPath = option("--start-lock");
const expectedCommandSuffix = option("--watcher-command-suffix");
const expectedSessionId = option("--session-id");
const expectedGeneration = option("--instance-token");
if (!eventsPath || !cursorPath || !expectedSessionId || !expectedGeneration) {
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
      expectedGeneration,
      waitMs,
    );
    process.stdout.write(`${JSON.stringify(result ?? { status: "no_event" })}\n`);
  } else if (process.argv.includes("--ack")) {
    if (!pidPath || !instanceTokenPath || !expectedCommandSuffix || !startLockPath) {
      throw new Error("--ack requires PID, instance-token, command-suffix, and start-lock identity");
    }
    acknowledge({
      eventsPath,
      cursorPath,
      nextCursor: Number(option("--ack")),
      expectedSessionId,
      expectedGeneration,
      pidPath,
      instanceTokenPath,
      expectedCommandSuffix,
      startLockPath,
    });
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
