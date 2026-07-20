import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  assertCanPost,
  assertExpectedIdentity,
  hasWriteAuthorization,
  parseIdentity,
  parsePermissions,
} from "./identity.mjs";

test("parseIdentity requires a login", () => {
  assert.deepEqual(parseIdentity({ login: "reviewer-bot" }), { login: "reviewer-bot" });
  assert.throws(() => parseIdentity({}), /missing login/);
});

test("parsePermissions normalizes missing fields to false", () => {
  assert.deepEqual(parsePermissions({}), { admin: false, maintain: false, push: false, triage: false, pull: false });
  assert.deepEqual(parsePermissions({ permissions: { pull: true } }), {
    admin: false,
    maintain: false,
    push: false,
    triage: false,
    pull: true,
  });
});

// --- F2: read access is NOT sufficient; only a write-capable role is ------

test("hasWriteAuthorization rejects read-only access (pull true, everything else false)", () => {
  assert.equal(hasWriteAuthorization({ pull: true, triage: true, push: false, maintain: false, admin: false }), false);
});

test("hasWriteAuthorization accepts push (the Write role)", () => {
  assert.equal(hasWriteAuthorization({ pull: true, push: true, maintain: false, admin: false }), true);
});

test("hasWriteAuthorization accepts maintain", () => {
  assert.equal(hasWriteAuthorization({ pull: true, push: false, maintain: true, admin: false }), true);
});

test("hasWriteAuthorization accepts admin", () => {
  assert.equal(hasWriteAuthorization({ pull: true, push: false, maintain: false, admin: true }), true);
});

test("hasWriteAuthorization rejects a fully empty permission set", () => {
  assert.equal(hasWriteAuthorization({}), false);
});

test("assertExpectedIdentity fails loud on a mismatched login", () => {
  const identity = { login: "reviewer-bot" };
  assert.equal(assertExpectedIdentity(identity, "reviewer-bot"), identity);
  assert.throws(() => assertExpectedIdentity(identity, "someone-else"), /unexpected GitHub identity/);
});

test("assertExpectedIdentity is a no-op when no expected login was given", () => {
  const identity = { login: "reviewer-bot" };
  assert.equal(assertExpectedIdentity(identity, undefined), identity);
});

test("assertCanPost fails closed when the identity has only read access", () => {
  assert.throws(() => assertCanPost({ pull: true, push: false }, "reviewer-bot"), /lacks write authorization/);
});

test("assertCanPost passes for a write-capable identity", () => {
  const permissions = { pull: true, push: true };
  assert.equal(assertCanPost(permissions, "reviewer-bot"), permissions);
});

const scriptPath = fileURLToPath(new URL("./identity.mjs", import.meta.url));
const identityFixture = fileURLToPath(new URL("../fixtures/identity-ok.json", import.meta.url));
const permissionsWrite = fileURLToPath(new URL("../fixtures/permissions-write.json", import.meta.url));
const permissionsMaintain = fileURLToPath(new URL("../fixtures/permissions-maintain.json", import.meta.url));
const permissionsAdmin = fileURLToPath(new URL("../fixtures/permissions-admin.json", import.meta.url));
const permissionsReadOnly = fileURLToPath(new URL("../fixtures/permissions-read-only.json", import.meta.url));
const permissionsNone = fileURLToPath(new URL("../fixtures/permissions-none.json", import.meta.url));

function runIdentity(permissionsFixture, extraArgs = []) {
  return Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "AojdevStudio/agentic-utilities",
    "--identity-fixture",
    identityFixture,
    "--permissions-fixture",
    permissionsFixture,
    ...extraArgs,
  ]);
}

test("identity CLI passes for push (Write role)", () => {
  assert.equal(runIdentity(permissionsWrite).exitCode, 0);
});

test("identity CLI passes for maintain", () => {
  assert.equal(runIdentity(permissionsMaintain).exitCode, 0);
});

test("identity CLI passes for admin", () => {
  assert.equal(runIdentity(permissionsAdmin).exitCode, 0);
});

test("identity CLI fails closed for read-only access (the exact regression this round closes)", () => {
  assert.equal(runIdentity(permissionsReadOnly).exitCode, 1);
});

test("identity CLI fails closed when the identity has no access at all", () => {
  assert.equal(runIdentity(permissionsNone).exitCode, 1);
});

test("identity CLI fails loud on an unexpected identity", () => {
  const result = runIdentity(permissionsWrite, ["--expect-login", "someone-else"]);
  assert.equal(result.exitCode, 1);
});
