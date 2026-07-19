#!/usr/bin/env bun

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { paneFleetIdentity } from "./fleet-labels.mjs";

function option(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

function digestPath(repoRoot) {
  return createHash("sha256").update(path.resolve(repoRoot)).digest("hex");
}

export function fleetOwnerToken(repoRoot) {
  return digestPath(repoRoot).slice(0, 16);
}

function ownsPath(pane, repoRoot) {
  const value = pane.foreground_cwd ?? pane.cwd;
  if (!value) return false;
  const root = path.resolve(repoRoot);
  const candidate = path.resolve(value);
  return candidate === root || candidate.startsWith(`${root}${path.sep}`);
}

function provesOwnership(pane, repoRoot, ownerToken) {
  const identity = paneFleetIdentity(pane);
  if (!identity || !ownsPath(pane, repoRoot)) return false;
  return identity.source === "legacy-label" || identity.owner === ownerToken;
}

function occupants(panes, key) {
  return panes.filter((pane) => paneFleetIdentity(pane)?.key === key);
}

export function resolveProjectKey({ panes, baseKey, repoRoot }) {
  const ownerToken = fleetOwnerToken(repoRoot);
  const established = new Set(
    panes
      .filter((pane) => provesOwnership(pane, repoRoot, ownerToken))
      .map((pane) => paneFleetIdentity(pane)?.key)
      .filter((key) => key === baseKey || key?.startsWith(`${baseKey}-`)),
  );

  if (established.size > 1) {
    throw new Error(`multiple ownership-proven project keys: ${[...established].join(", ")}`);
  }
  if (established.size === 1) return [...established][0];
  if (occupants(panes, baseKey).length === 0) return baseKey;

  const digest = digestPath(repoRoot);
  for (const length of [4, 8, 12, 16, 24, 32, 64]) {
    const candidate = `${baseKey}-${digest.slice(0, length)}`;
    if (occupants(panes, candidate).length === 0) return candidate;
  }
  throw new Error("no unoccupied deterministic project key available");
}

function selfTest() {
  const fixture = JSON.parse(readFileSync(new URL("../fixtures/project-key-collision.json", import.meta.url), "utf8"));
  assert.equal(resolveProjectKey(fixture), fixture.expectedKey);

  const repoRoot = "/repos/new-billing-api";
  const digest = digestPath(repoRoot);
  assert.equal(
    resolveProjectKey({
      baseKey: "ba",
      repoRoot,
      panes: [
        { label: "ba-control-pane", cwd: "/repos/foreign" },
        { label: `ba-${digest.slice(0, 4)}-pi-impl`, cwd: "/repos/another-foreign" },
      ],
    }),
    `ba-${digest.slice(0, 8)}`,
  );
  assert.equal(
    resolveProjectKey({
      baseKey: "ba",
      repoRoot,
      panes: [
        {
          cwd: repoRoot,
          tokens: {
            fleet_key: "ba-owned",
            fleet_owner: fleetOwnerToken(repoRoot),
            fleet_kind: "worker",
          },
        },
      ],
    }),
    "ba-owned",
  );
  assert.throws(() =>
    resolveProjectKey({
      baseKey: "ba",
      repoRoot,
      panes: [
        { label: "ba-control-pane", cwd: repoRoot },
        { label: "ba-abcd-pi-impl", cwd: repoRoot },
      ],
    }),
  );
  process.stdout.write(`${JSON.stringify({ status: "pass", checks: 4 })}\n`);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.exit(0);
}

const baseKey = option("--base-key");
const repoRoot = option("--repo-root");
if (!baseKey || !repoRoot) {
  process.stderr.write(
    `${JSON.stringify({
      level: "fatal",
      event: "invalid_arguments",
      sessionId: "project-key-resolver",
      message: "resolve-project-key requires --base-key and --repo-root",
    })}\n`,
  );
  process.exit(2);
}

const panes = JSON.parse(await Bun.stdin.text());
const projectKey = resolveProjectKey({ panes, baseKey, repoRoot });
const result = { projectKey, ownerToken: fleetOwnerToken(repoRoot) };
process.stdout.write(process.argv.includes("--json") ? `${JSON.stringify(result)}\n` : `${projectKey}\n`);
