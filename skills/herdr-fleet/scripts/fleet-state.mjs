#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { paneFleetIdentity } from "./fleet-labels.mjs";

function extractPanes(payload) {
  const value = payload?.result?.panes ?? payload?.panes ?? payload?.result ?? payload;
  return Array.isArray(value) ? value : [];
}

function scopedPanes(payload, { workspaceId, tabId }) {
  return extractPanes(payload).filter((pane) => pane.workspace_id === workspaceId && pane.tab_id === tabId);
}

export function classifiedInventory(payload, scope) {
  return scopedPanes(payload, scope)
    .map((pane) => ({
      paneId: pane.pane_id,
      label: pane.label,
      cwd: pane.foreground_cwd ?? pane.cwd,
      identity: paneFleetIdentity(pane),
    }))
    .sort((left, right) => String(left.paneId).localeCompare(String(right.paneId)));
}

export function inventoryFingerprint(inventory) {
  return createHash("sha256").update(JSON.stringify(inventory)).digest("hex");
}

export function validateRosterLabels(roster, projectKey) {
  if (!Array.isArray(roster)) throw new Error("roster must be an array");
  const controlLabel = `${projectKey}-control-pane`.toLowerCase();
  const renderedLabels = new Set();
  for (const worker of roster) {
    const label = typeof worker.label === "string" ? worker.label.trim() : "";
    const commandValue = worker.launchCommand ?? worker.command;
    const command = typeof commandValue === "string" ? commandValue.trim() : "";
    if (!label || !command) throw new Error("each worker requires a label and launch command");
    const rendered = `${projectKey}-${label}`.toLowerCase();
    if (rendered === controlLabel) throw new Error(`reserved control label: ${label}`);
    if (renderedLabels.has(rendered)) throw new Error(`duplicate worker label: ${label}`);
    renderedLabels.add(rendered);
  }
  return roster;
}

export function mutationTargetMismatches(pane, expected) {
  const identity = paneFleetIdentity(pane) ?? {};
  const actual = {
    paneId: pane?.pane_id,
    workspaceId: pane?.workspace_id,
    tabId: pane?.tab_id,
    label: pane?.label,
    projectKey: identity.key,
    ownerToken: identity.owner,
    kind: identity.kind,
    role: identity.role,
  };
  return Object.keys(actual).filter((key) => expected[key] !== undefined && actual[key] !== expected[key]);
}

export function workerMetadataHash(expected) {
  const canonical = {
    ownerToken: expected.ownerToken,
    projectKey: expected.projectKey,
    label: expected.label,
    role: expected.role,
    assignment: expected.assignment ?? null,
    command: expected.command,
    placement: expected.placement,
  };
  return createHash("sha256").update(JSON.stringify(canonical)).digest("hex").slice(0, 24);
}

export function metadataMismatches(pane, expected) {
  const tokens = pane?.tokens ?? {};
  const required = {
    fleet_owner: expected.ownerToken,
    fleet_key: expected.projectKey,
    fleet_kind: "worker",
    fleet_label: expected.label,
    fleet_role: expected.role,
    fleet_command: expected.command,
    fleet_placement: expected.placement,
    fleet_metadata_sha: workerMetadataHash(expected),
  };
  if (expected.assignment) required.fleet_assignment = expected.assignment;
  const mismatches = Object.entries(required)
    .filter(([key, value]) => tokens[key] !== value)
    .map(([key]) => key);
  if (!expected.assignment && Object.hasOwn(tokens, "fleet_assignment")) mismatches.push("fleet_assignment");
  if (pane?.label !== `${expected.projectKey}-${expected.label}`) mismatches.push("pane_label");
  return mismatches;
}

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`invalid fleet-state JSON: ${error.message}`);
  }
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

async function main() {
  const payload = parseJson(await Bun.stdin.text());
  const workspaceId = option("--workspace-id");
  const tabId = option("--tab-id");
  if (process.argv.includes("--validate-roster")) {
    validateRosterLabels(payload, option("--project-key"));
    process.stdout.write(`${JSON.stringify(payload)}\n`);
    return;
  }
  if (process.argv.includes("--metadata-hash")) {
    process.stdout.write(`${workerMetadataHash(payload)}\n`);
    return;
  }
  if (process.argv.includes("--verify-target")) {
    const mismatches = mutationTargetMismatches(payload.pane, payload.expected);
    if (mismatches.length > 0) throw new Error(`mutation target changed: ${mismatches.join(", ")}`);
    process.stdout.write(`${JSON.stringify({ status: "verified" })}\n`);
    return;
  }
  if (process.argv.includes("--verify-metadata")) {
    const mismatches = metadataMismatches(payload.pane, payload.expected);
    if (mismatches.length > 0) throw new Error(`metadata mismatch: ${mismatches.join(", ")}`);
    process.stdout.write(`${JSON.stringify({ status: "verified" })}\n`);
    return;
  }
  if (!workspaceId || !tabId) throw new Error("inventory mode requires workspace and tab IDs");
  const scope = { workspaceId, tabId };
  const fingerprint = inventoryFingerprint(classifiedInventory(payload, scope));
  const expected = option("--assert-fingerprint");
  if (expected && expected !== fingerprint) throw new Error("fleet topology changed during roster intake");
  const result = process.argv.includes("--fingerprint") ? { fingerprint } : scopedPanes(payload, scope);
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({
        level: "fatal",
        event: "fleet_state_invalid",
        sessionId: "fleet-state",
        message: error.message,
      })}\n`,
    );
    process.exit(1);
  }
}
