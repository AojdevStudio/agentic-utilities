import { expect, test } from "bun:test";
import { type CheckRun, decideMerge, type HeadState, PASS_APP_ID, PASS_SIGNAL } from "./merge";

const run = (conclusion: string, appId: number | null = PASS_APP_ID): CheckRun => ({
  name: PASS_SIGNAL,
  status: "completed",
  conclusion,
  appId,
});

const head = (over: Partial<HeadState>): HeadState => ({
  repo: "AojdevStudio/agentic-utilities",
  pinned: true,
  sha: "a".repeat(40),
  parents: ["b".repeat(40)],
  runs: [],
  ...over,
});

test("a Pullfrog success on the head passes", () => {
  expect(decideMerge(head({ runs: [run("success")] }))).toEqual({ ok: true, via: "approval" });
});

test("a same-name success from another app does not pass", () => {
  expect(decideMerge(head({ runs: [run("success", 15368)] })).ok).toBe(false);
  expect(decideMerge(head({ runs: [run("failure"), run("success", 15368)] })).ok).toBe(false);
});

test("a same-name success from another app on the parent does not carry forward", () => {
  const conflictFix = { parents: ["b".repeat(40), "c".repeat(40)], mergesBase: true };
  expect(decideMerge(head({ ...conflictFix, parentRuns: [run("success", 15368)] })).ok).toBe(false);
});

test("parent approval carries forward to a base merge the bot has not judged", () => {
  const state = head({ parents: ["b".repeat(40), "c".repeat(40)], mergesBase: true, parentRuns: [run("success")] });
  expect(decideMerge(state)).toEqual({ ok: true, via: "conflict-fix" });
});

test("a Pullfrog rejection of the head vetoes parent approval", () => {
  const state = head({
    parents: ["b".repeat(40), "c".repeat(40)],
    mergesBase: true,
    runs: [run("failure")],
    parentRuns: [run("success")],
  });
  expect(decideMerge(state).ok).toBe(false);
});
