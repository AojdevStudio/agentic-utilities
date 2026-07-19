import { test } from "bun:test";
import assert from "node:assert/strict";
import {
  classifiedInventory,
  inventoryFingerprint,
  metadataMismatches,
  mutationTargetMismatches,
  validateRosterLabels,
} from "./fleet-state.mjs";

const fixture = await Bun.file(new URL("../fixtures/fleet-state-race.json", import.meta.url)).json();
const before = classifiedInventory(fixture.before, fixture.scope);
const afterRace = classifiedInventory(fixture.afterTopologyRace, fixture.scope);

test("topology races change the scoped inventory fingerprint", () => {
  assert.notEqual(inventoryFingerprint(before), inventoryFingerprint(afterRace));
});

test("inventory excludes panes from another tab", () => {
  assert.equal(before.length, 1);
});

test("worker roster rejects the reserved control label", () => {
  assert.throws(() => validateRosterLabels(fixture.rosterWithReservedLabel, "app"), /reserved control label/);
});

test("metadata readback detects a truncated command", () => {
  assert.deepEqual(metadataMismatches(fixture.metadataReadback.pane, fixture.metadataReadback.expected), [
    "fleet_command",
    "fleet_metadata_sha",
  ]);
});

const expectedMutationTarget = {
  paneId: "w1:p1",
  workspaceId: "w1",
  tabId: "w1:t1",
  projectKey: "app",
  ownerToken: "owner-1",
  kind: "worker",
  label: "app-builder",
  role: "implementer",
};

test("fresh pane readback detects label and role races before mutation", () => {
  const racedPane = fixture.afterTopologyRace.result.panes[0];
  assert.deepEqual(mutationTargetMismatches(racedPane, expectedMutationTarget), ["label", "role"]);
});

test("fresh pane readback detects scope and ownership races before mutation", () => {
  const racedPane = structuredClone(fixture.before.result.panes[0]);
  racedPane.workspace_id = "w2";
  racedPane.tokens.fleet_owner = "owner-2";
  assert.deepEqual(mutationTargetMismatches(racedPane, expectedMutationTarget), ["workspaceId", "ownerToken"]);
});
