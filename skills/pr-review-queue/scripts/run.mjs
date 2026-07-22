#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { createStopSignal } from "./poll-state.mjs";
import { runLoop } from "./run-loop.mjs";

export { loadObservations, pollOnce, runLoop, saveObservations } from "./run-loop.mjs";

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? fallback : value;
}

export function runScript(scriptName, args, acceptedStatuses = [0]) {
  const scriptPath = new URL(`./${scriptName}`, import.meta.url).pathname;
  const result = spawnSync("bun", [scriptPath, ...args], { encoding: "utf8", timeout: 30_000 });
  if (result.error) throw new Error(result.error.message);
  if (!acceptedStatuses.includes(result.status)) {
    throw new Error(`${scriptName} exited ${result.status}: ${(result.stderr || result.stdout || "").trim()}`);
  }
  try {
    return JSON.parse(result.stdout);
  } catch {
    throw new Error(`${scriptName} produced non-JSON output: ${(result.stderr || result.stdout || "").trim()}`);
  }
}

function currentHead(repository, number) {
  const result = spawnSync(
    "gh",
    ["pr", "view", String(number), "-R", repository, "--json", "headRefOid", "-q", ".headRefOid"],
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

function claimArgs(repository, pr, head, identities, staleMs) {
  return [
    "--repo",
    repository,
    "--pr",
    String(pr),
    "--expected-head",
    head,
    "--stale-ms",
    String(staleMs),
    "--authorized",
    identities.join(","),
  ];
}

function githubFetchPrState(repository, identities, staleMs) {
  return async (pr) => {
    const head = currentHead(repository, pr);
    const election = runScript("claim.mjs", claimArgs(repository, pr, head, identities, staleMs));
    const gateArgs = ["--repo", repository, "--pr", String(pr), "--expected-head", head];
    const gateEvidence = runScript("review-gate.mjs", gateArgs, [0, 3]);
    return { head, election, gateEvidence };
  };
}

export function sleepWithStop(ms, stopSignal) {
  return new Promise((resolve) => {
    let unsubscribe = () => {};
    const timer = setTimeout(finish, ms);
    function finish() {
      clearTimeout(timer);
      unsubscribe();
      resolve();
    }
    unsubscribe = stopSignal.subscribe(finish);
  });
}

function cliOptions() {
  const repository = option("--repo");
  const authorizedIdentities = (option("--authorized") ?? "").split(",").filter(Boolean);
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra) throw new Error("run requires --repo owner/name");
  if (authorizedIdentities.length === 0) throw new Error("run requires --authorized as a comma-separated list");
  const staleMs = Number(option("--stale-ms") ?? 20 * 60 * 1000);
  const maxErrors = Number(option("--max-errors") ?? 5);
  if (!Number.isFinite(staleMs) || staleMs < 0) throw new Error("run requires --stale-ms as a non-negative number");
  if (!Number.isSafeInteger(maxErrors) || maxErrors <= 0) {
    throw new Error("run requires --max-errors as a positive integer");
  }
  return {
    repository,
    authorizedIdentities,
    statePath: option("--state", ".pr-review-queue-state.json"),
    staleMs,
    maxErrors,
  };
}

async function main() {
  const { repository, authorizedIdentities, statePath, staleMs, maxErrors } = cliOptions();
  const stopSignal = createStopSignal();
  for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => stopSignal.requestStop(signal));
  const status = await runLoop({
    fetchQueue: githubFetchQueue(repository),
    fetchPrState: githubFetchPrState(repository, authorizedIdentities, staleMs),
    statePath,
    stopSignal,
    nowFn: () => new Date().toISOString(),
    sleepFn: sleepWithStop,
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
