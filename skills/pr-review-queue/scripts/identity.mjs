#!/usr/bin/env bun
import { spawnSync } from "node:child_process";

// GitHub identity and write-access verification, required before any
// external comment/review/claim post. Mirrors the standing rule that
// unattended gh surfaces must pin identity and fail loud rather than
// silently posting under the wrong account.

function parseJson(text, context) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${context}: ${error.message}`);
  }
}

/** Parse `gh api user` JSON into a minimal identity record. */
export function parseIdentity(payload) {
  if (!payload?.login) throw new Error("GitHub identity response missing login");
  return { login: payload.login };
}

/** Parse `gh api repos/{owner}/{repo}` permissions into a normalized capability set. */
export function parsePermissions(payload) {
  const permissions = payload?.permissions ?? {};
  return {
    admin: permissions.admin === true,
    maintain: permissions.maintain === true,
    push: permissions.push === true,
    triage: permissions.triage === true,
    pull: permissions.pull === true,
  };
}

/** Read access is sufficient to post reviews/comments; push/admin is not required for this skill. */
export function canPostExternalComments(permissions) {
  return permissions.pull === true;
}

/** Fail loud when the authenticated identity does not match what the dispatcher expected. */
export function assertExpectedIdentity(identity, expectedLogin) {
  if (expectedLogin && identity.login !== expectedLogin) {
    throw new Error(`unexpected GitHub identity: authenticated as ${identity.login}, expected ${expectedLogin}`);
  }
  return identity;
}

/** Fail loud when the authenticated identity cannot post to the target repository. */
export function assertCanPost(permissions, identityLogin) {
  if (!canPostExternalComments(permissions)) {
    throw new Error(`${identityLogin} lacks repository access to post external comments`);
  }
  return permissions;
}

function runGh(args) {
  const result = spawnSync("gh", args, { encoding: "utf8", timeout: 15_000 });
  if (result.error || result.status !== 0) {
    throw new Error(result.stderr?.trim() || result.error?.message || `gh ${args.join(" ")} failed`);
  }
  return parseJson(result.stdout, `invalid gh ${args.join(" ")} response`);
}

function option(name) {
  const index = process.argv.indexOf(name);
  const value = index === -1 ? undefined : process.argv[index + 1];
  return !value || value.startsWith("--") ? undefined : value;
}

async function main() {
  const repository = option("--repo");
  const expectedLogin = option("--expect-login");
  const identityFixture = option("--identity-fixture");
  const permissionsFixture = option("--permissions-fixture");
  const [owner, name, extra] = repository?.split("/") ?? [];
  if (!owner || !name || extra) throw new Error("identity requires --repo owner/name");

  const identityPayload = identityFixture
    ? parseJson(await Bun.file(identityFixture).text(), "invalid identity fixture")
    : runGh(["api", "user"]);
  const identity = assertExpectedIdentity(parseIdentity(identityPayload), expectedLogin);

  const permissionsPayload = permissionsFixture
    ? parseJson(await Bun.file(permissionsFixture).text(), "invalid permissions fixture")
    : runGh(["api", `repos/${owner}/${name}`]);
  const permissions = assertCanPost(parsePermissions(permissionsPayload), identity.login);

  process.stdout.write(`${JSON.stringify({ login: identity.login, repository, permissions, verified: true })}\n`);
}

if (import.meta.path === Bun.main) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({ level: "fatal", event: "identity_verification_failed", message: error.message })}\n`,
    );
    process.exit(1);
  }
}
