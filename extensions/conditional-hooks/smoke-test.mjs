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

  console.log("conditional-hooks smoke tests passed");
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
