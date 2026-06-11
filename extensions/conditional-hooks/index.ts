import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";

export type ConditionalHook = {
  name: string;
  enabled?: boolean;
  events?: string[];
  when?: Record<string, unknown>;
  run?: Record<string, unknown>;
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

const CONFIG_BASENAME = "conditional-hooks.json";
let latestState: ConditionalHooksState = emptyState();

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

  return {
    ...value,
    name: value.name.trim(),
    enabled: typeof value.enabled === "boolean" ? value.enabled : undefined,
    events: Array.isArray(value.events) ? [...value.events] : undefined,
    when: isRecord(value.when) ? { ...value.when } : undefined,
    run: isRecord(value.run) ? { ...value.run } : undefined,
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
