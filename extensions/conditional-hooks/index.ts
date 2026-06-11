import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";

export type ConditionalHook = {
  name: string;
  enabled?: boolean;
  events?: string[];
  when?: ConditionalHookWhen;
  run?: ConditionalHookRun;
  runOnError?: boolean;
};

export type ConditionalHookWhen = {
  toolName?: { regex?: string };
  command?: { regex?: string };
  repoContains?: string;
  [key: string]: unknown;
};

export type ConditionalHookRun = {
  command?: string;
  cwd?: "repoRoot" | string;
  timeout?: number;
  ignoreFailure?: boolean;
  runOnError?: boolean;
  appendToToolResult?: boolean;
  [key: string]: unknown;
};

type ConditionalHooksFile = {
  hooks: ConditionalHook[];
};

export type ConfigSourceStatus = {
  label: "global" | "project";
  path: string;
  status: "missing" | "skipped" | "loaded" | "invalid";
  hookNames: string[];
};

export type ConditionalHooksState = {
  sources: ConfigSourceStatus[];
  activeHooks: ConditionalHook[];
  disabledHooks: ConditionalHook[];
  warnings: string[];
};

type LoadOptions = {
  globalPath?: string;
  projectPath?: string;
};

type ExecLike = (
  command: string,
  args: string[],
  options?: { cwd?: string; timeout?: number },
) => Promise<{ stdout?: string; stderr?: string; code?: number; exitCode?: number; killed?: boolean }>;

type WarningSink = (warning: string, hook: ConditionalHook) => void | Promise<void>;

type ConditionalHookEvent = "tool_result" | "tool_execution_end";

type BashSource = {
  eventType: ConditionalHookEvent;
  toolCallId?: string;
  toolName: string;
  command: string;
  cwd: string;
  isError: boolean;
};

export type ConditionalHookRunResult = {
  hookName: string;
  stdout: string;
  warning?: string;
  appendToToolResult: boolean;
};

const CONFIG_BASENAME = "conditional-hooks.json";
const MAX_TRACKED_TOOL_CALLS = 500;
let latestState: ConditionalHooksState = emptyState();
const toolStartCache = new Map<string, BashSource>();
const completedHookRuns = new Set<string>();

function emptyState(): ConditionalHooksState {
  return {
    sources: [],
    activeHooks: [],
    disabledHooks: [],
    warnings: [],
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validateHook(value: unknown, sourcePath: string, index: number, warnings: string[]): ConditionalHook | null {
  if (!isRecord(value)) {
    warnings.push(`${sourcePath}: hooks[${index}] must be an object.`);
    return null;
  }

  if (typeof value.name !== "string" || value.name.trim() === "") {
    warnings.push(`${sourcePath}: hooks[${index}].name must be a non-empty string.`);
    return null;
  }

  if ("enabled" in value && typeof value.enabled !== "boolean") {
    warnings.push(`${sourcePath}: hook "${value.name}" enabled must be a boolean when present.`);
    return null;
  }

  if ("events" in value) {
    if (
      !Array.isArray(value.events) ||
      value.events.some((event) => typeof event !== "string" || event.trim() === "")
    ) {
      warnings.push(`${sourcePath}: hook "${value.name}" events must be an array of non-empty strings when present.`);
      return null;
    }
  }

  if ("when" in value && !isRecord(value.when)) {
    warnings.push(`${sourcePath}: hook "${value.name}" when must be an object when present.`);
    return null;
  }

  if ("run" in value && !isRecord(value.run)) {
    warnings.push(`${sourcePath}: hook "${value.name}" run must be an object when present.`);
    return null;
  }

  if ("runOnError" in value && typeof value.runOnError !== "boolean") {
    warnings.push(`${sourcePath}: hook "${value.name}" runOnError must be a boolean when present.`);
    return null;
  }

  return {
    ...value,
    name: value.name.trim(),
    enabled: typeof value.enabled === "boolean" ? value.enabled : undefined,
    events: Array.isArray(value.events) ? [...value.events] : undefined,
    when: isRecord(value.when) ? { ...value.when } : undefined,
    run: isRecord(value.run) ? { ...value.run } : undefined,
    runOnError: typeof value.runOnError === "boolean" ? value.runOnError : undefined,
  };
}

function readHooksFile(sourcePath: string, warnings: string[]): ConditionalHooksFile | null {
  let raw: string;
  try {
    raw = fs.readFileSync(sourcePath, "utf8");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warnings.push(`${sourcePath}: unable to read config: ${message}`);
    return null;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    warnings.push(`${sourcePath}: invalid strict JSON: ${message}`);
    return null;
  }

  if (!isRecord(parsed) || !Array.isArray(parsed.hooks)) {
    warnings.push(`${sourcePath}: config must be a JSON object with a hooks array.`);
    return null;
  }

  const hooks = parsed.hooks
    .map((hook, index) => validateHook(hook, sourcePath, index, warnings))
    .filter((hook): hook is ConditionalHook => hook !== null);

  return { hooks };
}

function loadSource(
  label: "global" | "project",
  sourcePath: string,
  warnings: string[],
): {
  source: ConfigSourceStatus;
  hooks: ConditionalHook[];
} {
  try {
    fs.statSync(sourcePath);
  } catch (error) {
    const code = error && typeof error === "object" && "code" in error ? (error as { code?: unknown }).code : undefined;
    if (code === "ENOENT" || code === "ENOTDIR") {
      return {
        source: { label, path: sourcePath, status: "missing", hookNames: [] },
        hooks: [],
      };
    }

    const message = error instanceof Error ? error.message : String(error);
    warnings.push(`${sourcePath}: unable to read config: ${message}`);
    return {
      source: { label, path: sourcePath, status: "invalid", hookNames: [] },
      hooks: [],
    };
  }

  const loaded = readHooksFile(sourcePath, warnings);
  if (!loaded) {
    return {
      source: { label, path: sourcePath, status: "invalid", hookNames: [] },
      hooks: [],
    };
  }

  return {
    source: { label, path: sourcePath, status: "loaded", hookNames: loaded.hooks.map((hook) => hook.name) },
    hooks: loaded.hooks,
  };
}

export function loadConditionalHooksState(
  cwd: string,
  projectTrusted: boolean,
  options: LoadOptions = {},
): ConditionalHooksState {
  const warnings: string[] = [];
  const sources: ConfigSourceStatus[] = [];
  const merged = new Map<string, ConditionalHook>();

  const globalPath = options.globalPath ?? path.join(os.homedir(), ".pi", "agent", CONFIG_BASENAME);
  const globalSource = loadSource("global", globalPath, warnings);
  sources.push(globalSource.source);
  for (const hook of globalSource.hooks) merged.set(hook.name, hook);

  const projectPath = options.projectPath ?? path.join(cwd, ".pi", CONFIG_BASENAME);
  if (projectTrusted) {
    const projectSource = loadSource("project", projectPath, warnings);
    sources.push(projectSource.source);
    for (const hook of projectSource.hooks) merged.set(hook.name, hook);
  } else {
    sources.push({ label: "project", path: projectPath, status: "skipped", hookNames: [] });
  }

  const hooks = Array.from(merged.values()).sort((a, b) => a.name.localeCompare(b.name));
  return {
    sources,
    activeHooks: hooks.filter((hook) => hook.enabled !== false),
    disabledHooks: hooks.filter((hook) => hook.enabled === false),
    warnings,
  };
}

function isProjectTrusted(ctx: ExtensionContext): boolean {
  const candidate = ctx as ExtensionContext & {
    isProjectTrusted?: () => boolean;
    settingsManager?: { isProjectTrusted?: () => boolean };
  };

  const trustReaders: Array<{ fn: (() => boolean) | undefined; receiver: unknown }> = [
    { fn: candidate.isProjectTrusted, receiver: candidate },
    { fn: candidate.settingsManager?.isProjectTrusted, receiver: candidate.settingsManager },
  ];

  for (const { fn, receiver } of trustReaders) {
    if (!fn) continue;
    try {
      if (fn.call(receiver) === true) return true;
    } catch {
      return false;
    }
  }

  return false;
}

function formatNames(hooks: ConditionalHook[]): string {
  return hooks.length ? hooks.map((hook) => hook.name).join(", ") : "(none)";
}

export function extractBashSource(
  event: unknown,
  ctxCwd?: string,
  eventType: ConditionalHookEvent = "tool_result",
): BashSource | null {
  const record = isRecord(event) ? event : {};
  const toolName = typeof record.toolName === "string" ? record.toolName : "";
  if (!isBashLikeToolName(toolName)) return null;

  const input = isRecord(record.input) ? record.input : undefined;
  const args = isRecord(record.args) ? record.args : undefined;
  const toolInput = isRecord(record.tool_input) ? record.tool_input : undefined;

  const command = firstString(input?.command, args?.command, toolInput?.command);
  if (!command) return null;

  return {
    eventType,
    toolCallId: typeof record.toolCallId === "string" ? record.toolCallId : undefined,
    toolName,
    command,
    cwd: firstString(input?.cwd, args?.cwd, toolInput?.cwd, ctxCwd, ".") ?? ".",
    isError: record.isError === true,
  };
}

function firstString(...values: unknown[]): string | undefined {
  for (const value of values) {
    if (typeof value === "string" && value.trim() !== "") return value;
  }
  return undefined;
}

function isBashLikeToolName(toolName: string): boolean {
  return toolName === "bash" || toolName === "Bash" || toolName.endsWith(".bash");
}

function regexMatches(pattern: string | undefined, value: string): boolean {
  if (!pattern) return true;
  try {
    return new RegExp(pattern).test(value);
  } catch {
    return false;
  }
}

async function resolveRepoRoot(cwd: string, exec: ExecLike): Promise<string | null> {
  try {
    const result = await exec("git", ["-C", cwd, "rev-parse", "--show-toplevel"], { timeout: 10_000 });
    const repoRoot = String(result.stdout ?? "").trim();
    return repoRoot || null;
  } catch {
    return null;
  }
}

async function hookMatchesSource(
  hook: ConditionalHook,
  source: BashSource,
  exec: ExecLike,
): Promise<{ matches: boolean; repoRoot: string | null }> {
  if (!hook.events?.includes(source.eventType)) return { matches: false, repoRoot: null };

  const runOnError = hook.runOnError === true || hook.run?.runOnError === true;
  if (source.isError && !runOnError) return { matches: false, repoRoot: null };

  if (!regexMatches(hook.when?.toolName?.regex, source.toolName)) return { matches: false, repoRoot: null };
  if (!regexMatches(hook.when?.command?.regex, source.command)) return { matches: false, repoRoot: null };

  if (hook.when?.repoContains) {
    const repoRoot = await resolveRepoRoot(source.cwd, exec);
    if (!repoRoot) return { matches: false, repoRoot: null };
    if (!fs.existsSync(path.join(repoRoot, hook.when.repoContains))) return { matches: false, repoRoot };
    return { matches: true, repoRoot };
  }

  return { matches: true, repoRoot: null };
}

function markHookRun(source: BashSource, hook: ConditionalHook, dedupe?: Set<string>): boolean {
  if (!source.toolCallId || !dedupe) return true;
  const key = `${source.toolCallId}:${hook.name}`;
  if (dedupe.has(key)) return false;
  dedupe.add(key);
  pruneTrackedSet(dedupe);
  return true;
}

function pruneTrackedSet(set: Set<string>): void {
  while (set.size > MAX_TRACKED_TOOL_CALLS) {
    const oldest = set.values().next().value;
    if (typeof oldest !== "string") break;
    set.delete(oldest);
  }
}

function pruneToolStartCache(): void {
  while (toolStartCache.size > MAX_TRACKED_TOOL_CALLS) {
    const oldest = toolStartCache.keys().next().value;
    if (typeof oldest !== "string") break;
    cleanupToolCallTracking(oldest);
  }
}

function cleanupToolCallTracking(toolCallId: string): void {
  toolStartCache.delete(toolCallId);
  for (const key of Array.from(completedHookRuns)) {
    if (key.startsWith(`${toolCallId}:`)) completedHookRuns.delete(key);
  }
}

export async function runMatchingConditionalHooks(options: {
  hooks: ConditionalHook[];
  event: unknown;
  ctxCwd?: string;
  exec: ExecLike;
  warn?: WarningSink;
  source?: BashSource;
  dedupe?: Set<string>;
}): Promise<ConditionalHookRunResult[]> {
  const source = options.source ?? extractBashSource(options.event, options.ctxCwd);
  if (!source) return [];

  const results: ConditionalHookRunResult[] = [];
  for (const hook of options.hooks) {
    const { matches, repoRoot } = await hookMatchesSource(hook, source, options.exec);
    if (!matches) continue;
    if (!markHookRun(source, hook, options.dedupe)) continue;

    const command = hook.run?.command;
    if (typeof command !== "string" || command.trim() === "") continue;

    const cwd =
      hook.run?.cwd === "repoRoot"
        ? (repoRoot ?? (await resolveRepoRoot(source.cwd, options.exec)) ?? source.cwd)
        : source.cwd;
    try {
      const result = await options.exec("sh", ["-lc", command], { cwd, timeout: hook.run?.timeout });
      const exitCode = typeof result.code === "number" ? result.code : result.exitCode;
      const stdout = String(result.stdout ?? "");
      if (exitCode && exitCode !== 0) {
        if (hook.run?.ignoreFailure === true) continue;
        const stderr = String(result.stderr ?? "").trim();
        const warning = `Conditional Hook "${hook.name}" failed with exit code ${exitCode}${stderr ? `: ${stderr}` : ""}`;
        await options.warn?.(warning, hook);
        results.push({
          hookName: hook.name,
          stdout: "",
          warning,
          appendToToolResult: hook.run?.appendToToolResult === true,
        });
        continue;
      }
      results.push({ hookName: hook.name, stdout, appendToToolResult: hook.run?.appendToToolResult === true });
    } catch (error) {
      if (hook.run?.ignoreFailure === true) continue;
      const message = error instanceof Error ? error.message : String(error);
      const warning = `Conditional Hook "${hook.name}" failed: ${message}`;
      await options.warn?.(warning, hook);
      results.push({
        hookName: hook.name,
        stdout: "",
        warning,
        appendToToolResult: hook.run?.appendToToolResult === true,
      });
    }
  }
  return results;
}

export function formatHookOutputSections(results: ConditionalHookRunResult[]): string {
  return results
    .filter((result) => result.stdout.trim() !== "")
    .map((result) => `[${result.hookName}]\n${result.stdout.trimEnd()}`)
    .join("\n\n");
}

function appendHookOutputToContent(
  content: Array<{ type: string; text?: string }>,
  results: ConditionalHookRunResult[],
) {
  const appendable = results.filter((result) => result.appendToToolResult && result.stdout.trim() !== "");
  if (!appendable.length) return content;

  const hookText = formatHookOutputSections(appendable);
  const nextContent = content.map((item) => ({ ...item }));
  for (let index = nextContent.length - 1; index >= 0; index--) {
    const item = nextContent[index];
    if (item?.type === "text" && typeof item.text === "string") {
      item.text = `${item.text}\n\n${hookText}`;
      return nextContent;
    }
  }
  nextContent.push({ type: "text", text: hookText });
  return nextContent;
}

export function formatConditionalHooksStatus(state: ConditionalHooksState): string {
  const lines = [
    "Conditional Hook status",
    "",
    "Config sources:",
    ...state.sources.map((source) => {
      const hooks = source.hookNames.length ? ` [${source.hookNames.join(", ")}]` : "";
      return `- ${source.label}: ${source.status} ${source.path}${hooks}`;
    }),
    "",
    `Active hooks: ${formatNames(state.activeHooks)}`,
    `Disabled hooks: ${formatNames(state.disabledHooks)}`,
    "",
    "Warnings:",
    ...(state.warnings.length ? state.warnings.map((warning) => `- ${warning}`) : ["- (none)"]),
  ];

  return lines.join("\n");
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (_event, ctx) => {
    latestState = loadConditionalHooksState(ctx.cwd, isProjectTrusted(ctx));
  });

  pi.on("tool_execution_start", async (event, ctx) => {
    const source = extractBashSource(event, ctx.cwd, "tool_execution_end");
    if (source?.toolCallId) {
      toolStartCache.set(source.toolCallId, source);
      pruneToolStartCache();
    }
  });

  pi.on("tool_result", async (event, ctx) => {
    const results = await runMatchingConditionalHooks({
      hooks: latestState.activeHooks,
      event,
      ctxCwd: ctx.cwd,
      exec: (command, args, options) => pi.exec(command, args, options as any),
      dedupe: completedHookRuns,
      warn: async (warning, hook) => {
        latestState.warnings.push(warning);
        if (ctx.hasUI) ctx.ui.notify(warning, "warning");
        pi.sendMessage({
          customType: "conditional-hook-warning",
          content: warning,
          display: true,
          details: { hook: hook.name, timestamp: Date.now() },
        });
      },
    });

    const content = appendHookOutputToContent(event.content, results);
    if (content !== event.content) return { content: content as typeof event.content };
  });

  pi.on("tool_execution_end", async (event, ctx) => {
    const record: Record<string, unknown> = isRecord(event as unknown)
      ? (event as unknown as Record<string, unknown>)
      : {};
    const toolCallId = typeof record.toolCallId === "string" ? record.toolCallId : undefined;
    const cached = toolCallId ? toolStartCache.get(toolCallId) : undefined;
    const source = cached
      ? { ...cached, eventType: "tool_execution_end" as const, isError: record.isError === true }
      : extractBashSource(event, ctx.cwd, "tool_execution_end");
    const warnings: string[] = [];
    const results = await runMatchingConditionalHooks({
      hooks: latestState.activeHooks,
      event,
      ctxCwd: ctx.cwd,
      source: source ?? undefined,
      exec: (command, args, options) => pi.exec(command, args, options as any),
      dedupe: completedHookRuns,
      warn: async (warning) => {
        latestState.warnings.push(warning);
        warnings.push(warning);
        if (ctx.hasUI) ctx.ui.notify(warning, "warning");
      },
    });

    if (toolCallId) cleanupToolCallTracking(toolCallId);

    const output = formatHookOutputSections(results);
    const message = [output, ...warnings].filter(Boolean).join("\n\n");
    if (message) {
      pi.sendMessage({
        customType: "conditional-hook-output",
        content: message,
        display: true,
        details: { toolCallId, timestamp: Date.now() },
      });
    }
  });

  pi.registerCommand("conditional-hooks", {
    description: "Show Conditional Hook config sources, hooks, and warnings.",
    handler: async (_args, ctx) => {
      latestState = loadConditionalHooksState(ctx.cwd, isProjectTrusted(ctx));
      const message = formatConditionalHooksStatus(latestState);
      if (ctx.hasUI) {
        ctx.ui.notify(message, latestState.warnings.length ? "warning" : "info");
      } else {
        console.log(message);
      }
    },
  });
}
