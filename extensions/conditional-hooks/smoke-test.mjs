import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createJiti } from "@mariozechner/jiti";

const jiti = createJiti(import.meta.url);
const conditionalHooks = await jiti.import("./index.ts");

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "conditional-hooks-"));
try {
  const cwd = path.join(tmp, "repo");
  const globalPath = path.join(tmp, "home", ".pi", "agent", "conditional-hooks.json");
  const projectPath = path.join(cwd, ".pi", "conditional-hooks.json");
  fs.mkdirSync(cwd, { recursive: true });

  writeJson(globalPath, {
    hooks: [
      { name: "global-active", enabled: true, events: ["tool_result"], when: {}, run: {} },
      { name: "overridden", enabled: true, events: ["tool_result"], when: {}, run: {} },
    ],
  });
  writeJson(projectPath, {
    hooks: [
      { name: "overridden", enabled: false, events: ["tool_result"], when: {}, run: {} },
      { name: "project-active", enabled: true, events: ["tool_result"], when: {}, run: {} },
    ],
  });

  const untrusted = conditionalHooks.loadConditionalHooksState(cwd, false, { globalPath, projectPath });
  assert.deepEqual(
    untrusted.activeHooks.map((hook) => hook.name),
    ["global-active", "overridden"],
    "untrusted project config must not add or override hooks",
  );
  assert.deepEqual(untrusted.disabledHooks, [], "untrusted project disable must be ignored");
  assert.equal(untrusted.sources.find((source) => source.label === "project")?.status, "skipped");

  const trusted = conditionalHooks.loadConditionalHooksState(cwd, true, { globalPath, projectPath });
  assert.deepEqual(
    trusted.activeHooks.map((hook) => hook.name),
    ["global-active", "project-active"],
    "trusted project config adds hooks and disables same-named globals",
  );
  assert.deepEqual(
    trusted.disabledHooks.map((hook) => hook.name),
    ["overridden"],
  );

  fs.writeFileSync(projectPath, "{ bad json", "utf8");
  const invalid = conditionalHooks.loadConditionalHooksState(cwd, true, { globalPath, projectPath });
  assert.equal(invalid.sources.find((source) => source.label === "project")?.status, "invalid");
  assert.match(invalid.warnings.join("\n"), /invalid strict JSON/);
  assert.match(conditionalHooks.formatConditionalHooksStatus(invalid), /Warnings:/);

  fs.rmSync(globalPath, { force: true });
  fs.mkdirSync(globalPath, { recursive: true });
  const unreadable = conditionalHooks.loadConditionalHooksState(cwd, false, { globalPath, projectPath });
  assert.equal(unreadable.sources.find((source) => source.label === "global")?.status, "invalid");
  assert.match(unreadable.warnings.join("\n"), /unable to read config/);

  const registered = { events: new Map(), commands: new Map() };
  const fakePi = {
    on(eventName, handler) {
      registered.events.set(eventName, handler);
    },
    registerCommand(commandName, config) {
      registered.commands.set(commandName, config);
    },
  };
  conditionalHooks.default(fakePi);
  assert.equal(typeof registered.events.get("session_start"), "function");
  assert.equal(typeof registered.commands.get("conditional-hooks")?.handler, "function");

  const commandCwd = path.join(tmp, "command-repo");
  const commandProjectPath = path.join(commandCwd, ".pi", "conditional-hooks.json");
  fs.mkdirSync(commandCwd, { recursive: true });
  writeJson(commandProjectPath, {
    hooks: [{ name: "trusted-command-hook", enabled: true, events: ["tool_result"], when: {}, run: {} }],
  });

  let logged = "";
  const originalLog = console.log;
  console.log = (message) => {
    logged = String(message);
  };
  try {
    await registered.commands.get("conditional-hooks").handler("", {
      cwd: commandCwd,
      hasUI: false,
      isProjectTrusted: () => true,
    });
  } finally {
    console.log = originalLog;
  }
  assert.match(logged, /project: loaded/);
  assert.match(logged, /trusted-command-hook/);

  const mergeRegex = "\\bgh\\s+pr\\s+merge\\b|\\bgit\\s+merge(?!-)\\b";
  const bashHook = {
    name: "bash-hook",
    enabled: true,
    events: ["tool_result"],
    when: {
      toolName: { regex: "(^|\\.)bash$|^Bash$" },
      command: { regex: mergeRegex },
    },
    run: { command: "echo hook", ignoreFailure: true },
  };

  async function runHookScenario({ event, hooks = [bashHook], repoRoot, failShell = false }) {
    const calls = [];
    const warnings = [];
    const exec = async (command, args, options = {}) => {
      calls.push({ command, args, options });
      if (command === "git") {
        if (!repoRoot) throw new Error("not a git repo");
        return { stdout: `${repoRoot}\n` };
      }
      if (command === "sh" && failShell) return { stdout: "", stderr: "hook failed", code: 1 };
      return { stdout: "hook output\n", stderr: "", code: 0 };
    };
    await conditionalHooks.runMatchingConditionalHooks({
      hooks,
      event,
      ctxCwd: cwd,
      exec,
      warn: (warning) => warnings.push(warning),
    });
    return { calls, warnings };
  }

  for (const toolName of ["bash", "Bash", "functions.bash"]) {
    const { calls } = await runHookScenario({
      event: { type: "tool_result", toolName, input: { command: "git merge --ff-only HEAD", cwd }, isError: false },
    });
    assert.equal(
      calls.some((call) => call.command === "sh"),
      true,
      `${toolName} should trigger bash hook`,
    );
  }

  const ghMerge = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "gh pr merge 123 --squash", cwd },
      isError: false,
    },
  });
  assert.equal(
    ghMerge.calls.some((call) => call.command === "sh"),
    true,
    "gh pr merge should trigger",
  );

  const argsCommand = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "bash",
      args: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  assert.equal(
    argsCommand.calls.some((call) => call.command === "sh"),
    true,
    "event.args.command should trigger",
  );

  const toolInputCommand = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "bash",
      tool_input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  assert.equal(
    toolInputCommand.calls.some((call) => call.command === "sh"),
    true,
    "event.tool_input.command should trigger",
  );

  const toolInputCwd = path.join(tmp, "tool-input-cwd");
  const toolInputCwdResult = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "bash",
      tool_input: { command: "git merge --ff-only HEAD", cwd: toolInputCwd },
      isError: false,
    },
  });
  assert.equal(
    toolInputCwdResult.calls.find((call) => call.command === "sh")?.options.cwd,
    toolInputCwd,
    "event.tool_input.cwd should be used when input/args cwd are absent",
  );

  for (const command of ["git merge-base --is-ancestor HEAD origin/main", "ls"]) {
    const { calls } = await runHookScenario({
      event: { type: "tool_result", toolName: "bash", input: { command, cwd }, isError: false },
    });
    assert.equal(
      calls.some((call) => call.command === "sh"),
      false,
      `${command} should not trigger`,
    );
  }

  const nonBash = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "read",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  assert.equal(nonBash.calls.length, 0, "non-bash tools must not trigger bash hooks");

  const repoRoot = path.join(tmp, "repo-root");
  const repoHook = {
    ...bashHook,
    name: "repo-hook",
    when: { ...bashHook.when, repoContains: "scripts/worktree-gc.ts" },
    run: { command: "pwd", cwd: "repoRoot", ignoreFailure: true },
  };
  const outsideRepo = await runHookScenario({
    hooks: [repoHook],
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  assert.equal(
    outsideRepo.calls.some((call) => call.command === "sh"),
    false,
    "repoContains skips outside repos",
  );

  fs.mkdirSync(repoRoot, { recursive: true });
  const missingRepoFile = await runHookScenario({
    hooks: [repoHook],
    repoRoot,
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  assert.equal(
    missingRepoFile.calls.some((call) => call.command === "sh"),
    false,
    "repoContains skips missing files",
  );

  fs.mkdirSync(path.join(repoRoot, "scripts"), { recursive: true });
  fs.writeFileSync(path.join(repoRoot, "scripts", "worktree-gc.ts"), "", "utf8");
  const repoMatch = await runHookScenario({
    hooks: [repoHook],
    repoRoot,
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
  });
  const shellCall = repoMatch.calls.find((call) => call.command === "sh");
  assert.equal(shellCall?.options.cwd, repoRoot, "run.cwd repoRoot should run from resolved git root");

  const defaultErrorSkip = await runHookScenario({
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: true,
    },
  });
  assert.equal(
    defaultErrorSkip.calls.some((call) => call.command === "sh"),
    false,
    "failed source skips by default",
  );

  const runOnError = await runHookScenario({
    hooks: [{ ...bashHook, runOnError: true }],
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: true,
    },
  });
  assert.equal(
    runOnError.calls.some((call) => call.command === "sh"),
    true,
    "runOnError true runs after failed source",
  );

  const failingEvent = {
    type: "tool_result",
    toolName: "bash",
    input: { command: "git merge --ff-only HEAD", cwd },
    isError: false,
  };
  const swallowedFailure = await runHookScenario({
    hooks: [{ ...bashHook, run: { command: "exit 1", ignoreFailure: true } }],
    event: {
      type: "tool_result",
      toolName: "bash",
      input: { command: "git merge --ff-only HEAD", cwd },
      isError: false,
    },
    failShell: true,
  });
  assert.deepEqual(swallowedFailure.warnings, [], "ignoreFailure true should swallow hook failures");

  const failure = await runHookScenario({
    hooks: [{ ...bashHook, run: { command: "exit 1", ignoreFailure: false } }],
    event: failingEvent,
    failShell: true,
  });
  assert.equal(failingEvent.isError, false, "hook failure must not mutate original result isError");
  assert.match(failure.warnings.join("\n"), /Conditional Hook "bash-hook" failed/);

  console.log("conditional-hooks smoke tests passed");
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
