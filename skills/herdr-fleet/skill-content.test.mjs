import { test } from "bun:test";
import assert from "node:assert/strict";

const read = (path) => Bun.file(new URL(path, import.meta.url)).text();

const [launch, skillReadme, skillsReadme, globalRules] = await Promise.all([
  read("./launch-fleet.md"),
  read("./README.md"),
  read("../README.md"),
  read("../../rules/global-skills.md"),
]);

test("global-canonical source of truth is explicit and consistent", () => {
  for (const text of [skillReadme, skillsReadme]) {
    assert.match(text, /~\/.agents\/skills\/herdr-fleet/);
  }
  assert.match(globalRules, /~\/.agents\/skills\/<name>\//);
  assert.doesNotMatch(skillsReadme, /HERDR_ENV/);
});

test("implementer worktrees receive collision-resistant identities", () => {
  assert.match(launch, /WORKTREE_ID=.*randomUUID/);
  assert.match(launch, /WORKTREE_BRANCH=/);
  assert.match(launch, /WORKTREE_PATH=/);
  assert.match(launch, /git worktree add -b "\$WORKTREE_BRANCH" "\$WORKTREE_PATH" "origin\/\$BASE_BRANCH"/);
  assert.match(launch, /record.*WORKTREE_ID.*WORKTREE_BRANCH.*WORKTREE_PATH/is);
});
