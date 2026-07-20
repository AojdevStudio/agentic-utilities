import { test } from "bun:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  assertCanPost,
  assertExpectedIdentity,
  canPostExternalComments,
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

test("canPostExternalComments requires only read access, not write", () => {
  assert.equal(canPostExternalComments({ pull: true, push: false }), true);
  assert.equal(canPostExternalComments({ pull: false, push: true }), false);
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

test("assertCanPost fails loud when the identity cannot post", () => {
  assert.throws(() => assertCanPost({ pull: false }, "reviewer-bot"), /lacks repository access/);
  const permissions = { pull: true };
  assert.equal(assertCanPost(permissions, "reviewer-bot"), permissions);
});

const scriptPath = fileURLToPath(new URL("./identity.mjs", import.meta.url));
const identityFixture = fileURLToPath(new URL("../fixtures/identity-ok.json", import.meta.url));
const permissionsWrite = fileURLToPath(new URL("../fixtures/permissions-write.json", import.meta.url));
const permissionsNone = fileURLToPath(new URL("../fixtures/permissions-none.json", import.meta.url));

test("identity CLI verifies and reports login + permissions", () => {
  const result = Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "AojdevStudio/agentic-utilities",
    "--identity-fixture",
    identityFixture,
    "--permissions-fixture",
    permissionsWrite,
  ]);
  assert.equal(result.exitCode, 0);
});

test("identity CLI fails loud when the identity lacks read access", () => {
  const result = Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "AojdevStudio/agentic-utilities",
    "--identity-fixture",
    identityFixture,
    "--permissions-fixture",
    permissionsNone,
  ]);
  assert.equal(result.exitCode, 1);
});

test("identity CLI fails loud on an unexpected identity", () => {
  const result = Bun.spawnSync([
    "bun",
    scriptPath,
    "--repo",
    "AojdevStudio/agentic-utilities",
    "--expect-login",
    "someone-else",
    "--identity-fixture",
    identityFixture,
    "--permissions-fixture",
    permissionsWrite,
  ]);
  assert.equal(result.exitCode, 1);
});
