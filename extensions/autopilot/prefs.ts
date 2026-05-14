import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import * as path from "node:path";
import type { ExtensionContext } from "@mariozechner/pi-coding-agent";

export type SourcePriority = "plan" | "github" | "linear";
export type VerificationProfile = "conservative" | "normal" | "strict";

export type RepoAutopilotConfig = {
  enabled?: boolean;
  source_priority?: SourcePriority[];
  verification_profile?: VerificationProfile;
  allow_paths?: string[];
  deny_paths?: string[];
  runner_command_template?: string;
  runner_concurrency?: number;
  runner_max_repair_attempts?: number;
  runner_idle_timeout_seconds?: number;
  runner_env_allowlist?: string[];
};

export type RepoAutopilotPreferences = {
  sourcePriority: SourcePriority[];
  verificationProfile: VerificationProfile;
  allowPaths: string[];
  denyPaths: string[];
  runnerCommandTemplate: string;
  runnerConcurrency: number;
  runnerMaxRepairAttempts: number;
  runnerIdleTimeoutSeconds: number;
  runnerEnvAllowlist: string[];
};

export type AutopilotSetupOptions = {
  enabled?: boolean;
  sourcePriority?: SourcePriority[];
  verificationProfile?: VerificationProfile;
  allowPaths?: string[];
  denyPaths?: string[];
  runnerCommandTemplate?: string;
  runnerConcurrency?: number;
  runnerMaxRepairAttempts?: number;
  runnerIdleTimeoutSeconds?: number;
  runnerEnvAllowlist?: string[];
};

export const DEFAULT_REPO_AUTOPILOT_PREFERENCES: RepoAutopilotPreferences = {
  sourcePriority: ["plan", "github", "linear"],
  verificationProfile: "normal",
  allowPaths: ["packages/**", "app/**", "docs/**", "opensrc/**"],
  denyPaths: ["node_modules/**"],
  runnerCommandTemplate: "cd {{WORKTREE_PATH}} && pi -p @{{PROMPT_PATH}}",
  runnerConcurrency: 2,
  runnerMaxRepairAttempts: 2,
  runnerIdleTimeoutSeconds: 600,
  runnerEnvAllowlist: [
    "PATH",
    "HOME",
    "SHELL",
    "TMPDIR",
    "PI_*",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
  ],
};

export const HOMELAB_REPO_AUTOPILOT_PREFERENCES: RepoAutopilotPreferences = {
  sourcePriority: ["plan", "github", "linear"],
  verificationProfile: "normal",
  allowPaths: [
    "nas/**",
    "proxmox/**",
    "cloudflare/**",
    "dream-machine/**",
    "mac-mini-server/**",
    "devices/**",
    "dashboard/**",
    "tailscale/**",
    "kvms/**",
    "docs/**",
    "scripts/**",
    "tasks/**",
    "resources/**",
    "opsec/**",
    "opensrc/**",
  ],
  denyPaths: ["node_modules/**"],
  runnerCommandTemplate: DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerCommandTemplate,
  runnerConcurrency: DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerConcurrency,
  runnerMaxRepairAttempts: DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerMaxRepairAttempts,
  runnerIdleTimeoutSeconds: DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerIdleTimeoutSeconds,
  runnerEnvAllowlist: DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerEnvAllowlist,
};

export function getRepoAutopilotConfigPath(repoCwd: string): string {
  return path.join(repoCwd, ".pi", "autopilot", "config.yml");
}

export function getRepoAutopilotEnabledPath(repoCwd: string): string {
  return path.join(repoCwd, ".pi", "autopilot", "enabled");
}

export function loadRepoAutopilotConfig(repoCwd: string): RepoAutopilotConfig {
  const configPath = getRepoAutopilotConfigPath(repoCwd);
  if (!fs.existsSync(configPath)) return {};

  try {
    return parseFlatYaml(fs.readFileSync(configPath, "utf8")) as RepoAutopilotConfig;
  } catch {
    return {};
  }
}

export function loadRepoAutopilotPreferences(repoCwd: string): RepoAutopilotPreferences {
  const config = loadRepoAutopilotConfig(repoCwd);
  const defaults = getDefaultRepoAutopilotPreferences(repoCwd);
  return {
    sourcePriority: normalizeSourcePriority(config.source_priority) ?? defaults.sourcePriority,
    verificationProfile: normalizeVerificationProfile(config.verification_profile) ?? defaults.verificationProfile,
    allowPaths: ensureOpensrcAllowed(normalizeStringArray(config.allow_paths) ?? defaults.allowPaths),
    denyPaths: stripOpensrcDeny(normalizeStringArray(config.deny_paths) ?? defaults.denyPaths),
    runnerCommandTemplate: normalizeNonEmptyString(config.runner_command_template) ?? defaults.runnerCommandTemplate,
    runnerConcurrency: normalizePositiveInteger(config.runner_concurrency) ?? defaults.runnerConcurrency,
    runnerMaxRepairAttempts:
      normalizePositiveInteger(config.runner_max_repair_attempts) ?? defaults.runnerMaxRepairAttempts,
    runnerIdleTimeoutSeconds:
      normalizePositiveInteger(config.runner_idle_timeout_seconds) ?? defaults.runnerIdleTimeoutSeconds,
    runnerEnvAllowlist: normalizeStringArray(config.runner_env_allowlist) ?? defaults.runnerEnvAllowlist,
  };
}

export function repoAutopilotEnabled(repoCwd: string): boolean {
  const config = loadRepoAutopilotConfig(repoCwd);
  return config.enabled === true || fs.existsSync(getRepoAutopilotEnabledPath(repoCwd));
}

export async function writeRepoAutopilotConfig(repoCwd: string, config: RepoAutopilotConfig): Promise<string> {
  const configPath = getRepoAutopilotConfigPath(repoCwd);
  await fsp.mkdir(path.dirname(configPath), { recursive: true });
  await fsp.writeFile(configPath, `${serializeRepoAutopilotConfig(config)}\n`, "utf8");
  return configPath;
}

export async function ensureRepoAutopilotEnabledMarker(repoCwd: string): Promise<string> {
  const enabledPath = getRepoAutopilotEnabledPath(repoCwd);
  await fsp.mkdir(path.dirname(enabledPath), { recursive: true });
  await fsp.writeFile(enabledPath, "enabled\n", "utf8");
  return enabledPath;
}

export function readGlobalAutopilotAllowlist(globalConfigPath: string): string[] {
  if (!fs.existsSync(globalConfigPath)) return [];
  const text = fs.readFileSync(globalConfigPath, "utf8");
  const lines = text.split(/\r?\n/);
  const allow: string[] = [];
  let sawRepos = false;
  let sawAllow = false;
  let reposIndent = 0;
  let allowIndent = 0;

  for (const line of lines) {
    const indent = leadingSpaces(line);
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    if (trimmed === "repos:") {
      sawRepos = true;
      sawAllow = false;
      reposIndent = indent;
      continue;
    }

    if (sawRepos && indent > reposIndent && trimmed === "allow:") {
      sawAllow = true;
      allowIndent = indent;
      continue;
    }

    if (sawAllow) {
      if (indent <= allowIndent) {
        sawAllow = false;
        continue;
      }

      if (trimmed.startsWith("- ")) {
        allow.push(trimmed.slice(2).trim());
      }
    }
  }

  return allow;
}

export async function ensureRepoInGlobalAutopilotAllowlist(
  globalConfigPath: string,
  repoPath: string,
): Promise<string> {
  const normalizedRepo = path.resolve(repoPath);
  const allowlist = readGlobalAutopilotAllowlist(globalConfigPath);
  if (!allowlist.includes(normalizedRepo)) {
    allowlist.push(normalizedRepo);
  }

  await fsp.mkdir(path.dirname(globalConfigPath), { recursive: true });
  await fsp.writeFile(globalConfigPath, `${serializeGlobalAutopilotConfig(allowlist)}\n`, "utf8");
  return globalConfigPath;
}

export async function setupAutopilotRepo(
  ctx: ExtensionContext,
  globalConfigPath: string,
): Promise<{
  configPath: string;
  enabledPath: string;
  allowlistPath: string;
  preferences: RepoAutopilotPreferences;
}> {
  const repoCwd = ctx.cwd;

  const sourceChoice = await ctx.ui.select("Autopilot source preference", [
    "Plan files first",
    "GitHub issues first",
    "Linear issues first",
    "Mixed / any source",
  ]);
  if (!sourceChoice) throw new Error("setup cancelled");

  const verificationChoice = await ctx.ui.select("Verification strictness", ["Conservative", "Normal", "Strict"]);
  if (!verificationChoice) throw new Error("setup cancelled");

  const pathPresetChoice = await ctx.ui.select("Allowed paths preset", [
    "Repo-aware default",
    "Repo-aware narrow",
    "Custom comma-separated globs",
  ]);
  if (!pathPresetChoice) throw new Error("setup cancelled");

  const options: AutopilotSetupOptions = {
    enabled: true,
    sourcePriority: mapSourcePriority(sourceChoice),
    verificationProfile: mapVerificationProfile(verificationChoice),
    allowPaths: await chooseAllowPaths(pathPresetChoice, ctx, repoCwd),
    denyPaths: ["node_modules/**"],
  };

  return setupAutopilotRepoWithOptions(repoCwd, globalConfigPath, options);
}

export async function setupAutopilotRepoWithOptions(
  repoCwd: string,
  globalConfigPath: string,
  options: AutopilotSetupOptions,
): Promise<{
  configPath: string;
  enabledPath: string;
  allowlistPath: string;
  preferences: RepoAutopilotPreferences;
}> {
  const preferences = normalizeSetupPreferences(options, repoCwd);

  const configPath = await writeRepoAutopilotConfig(repoCwd, {
    enabled: options.enabled ?? true,
    source_priority: preferences.sourcePriority,
    verification_profile: preferences.verificationProfile,
    allow_paths: preferences.allowPaths,
    deny_paths: preferences.denyPaths,
    runner_command_template: preferences.runnerCommandTemplate,
    runner_concurrency: preferences.runnerConcurrency,
    runner_max_repair_attempts: preferences.runnerMaxRepairAttempts,
    runner_idle_timeout_seconds: preferences.runnerIdleTimeoutSeconds,
    runner_env_allowlist: preferences.runnerEnvAllowlist,
  });
  const enabledPath = await ensureRepoAutopilotEnabledMarker(repoCwd);
  const allowlistPath = await ensureRepoInGlobalAutopilotAllowlist(globalConfigPath, repoCwd);

  return { configPath, enabledPath, allowlistPath, preferences };
}

function normalizeSetupPreferences(options: AutopilotSetupOptions, repoCwd?: string): RepoAutopilotPreferences {
  const defaults = getDefaultRepoAutopilotPreferences(repoCwd ?? process.cwd());
  return {
    sourcePriority: options.sourcePriority?.length ? options.sourcePriority : defaults.sourcePriority,
    verificationProfile: options.verificationProfile ?? defaults.verificationProfile,
    allowPaths: ensureOpensrcAllowed(options.allowPaths?.length ? options.allowPaths : defaults.allowPaths),
    denyPaths: stripOpensrcDeny(options.denyPaths?.length ? options.denyPaths : defaults.denyPaths),
    runnerCommandTemplate: options.runnerCommandTemplate?.trim() || defaults.runnerCommandTemplate,
    runnerConcurrency:
      options.runnerConcurrency && options.runnerConcurrency > 0
        ? Math.floor(options.runnerConcurrency)
        : defaults.runnerConcurrency,
    runnerMaxRepairAttempts:
      options.runnerMaxRepairAttempts && options.runnerMaxRepairAttempts > 0
        ? Math.floor(options.runnerMaxRepairAttempts)
        : defaults.runnerMaxRepairAttempts,
    runnerIdleTimeoutSeconds:
      options.runnerIdleTimeoutSeconds && options.runnerIdleTimeoutSeconds > 0
        ? Math.floor(options.runnerIdleTimeoutSeconds)
        : defaults.runnerIdleTimeoutSeconds,
    runnerEnvAllowlist: options.runnerEnvAllowlist?.length ? options.runnerEnvAllowlist : defaults.runnerEnvAllowlist,
  };
}

function getDefaultRepoAutopilotPreferences(repoCwd: string): RepoAutopilotPreferences {
  if (isHomelabRepo(repoCwd)) {
    return HOMELAB_REPO_AUTOPILOT_PREFERENCES;
  }
  return DEFAULT_REPO_AUTOPILOT_PREFERENCES;
}

function isHomelabRepo(repoCwd: string): boolean {
  return (
    fs.existsSync(path.join(repoCwd, "AGENTS.md")) &&
    fs.existsSync(path.join(repoCwd, "nas")) &&
    fs.existsSync(path.join(repoCwd, "proxmox"))
  );
}

function ensureOpensrcAllowed(paths: string[]): string[] {
  const normalized = dedupe(paths.map((item) => item.trim()).filter(Boolean));
  if (!normalized.includes("opensrc/**")) normalized.push("opensrc/**");
  return normalized;
}

function stripOpensrcDeny(paths: string[]): string[] {
  return dedupe(
    paths
      .map((item) => item.trim())
      .filter(Boolean)
      .filter((item) => item !== "opensrc/**"),
  );
}

function mapSourcePriority(choice: string): SourcePriority[] {
  if (choice === "GitHub issues first") return ["github", "linear", "plan"];
  if (choice === "Linear issues first") return ["linear", "github", "plan"];
  if (choice === "Mixed / any source") return ["plan", "github", "linear"];
  return ["plan", "github", "linear"];
}

function mapVerificationProfile(choice: string): VerificationProfile {
  if (choice === "Conservative") return "conservative";
  if (choice === "Strict") return "strict";
  return "normal";
}

async function chooseAllowPaths(choice: string, ctx: ExtensionContext, repoCwd: string): Promise<string[]> {
  const defaults = getDefaultRepoAutopilotPreferences(repoCwd);
  if (choice === "Repo-aware narrow") {
    if (isHomelabRepo(repoCwd)) {
      return ["nas/**", "proxmox/**", "cloudflare/**", "dream-machine/**", "docs/**", "scripts/**", "opensrc/**"];
    }
    return ["packages/**", "app/**", "opensrc/**"];
  }
  if (choice === "Custom comma-separated globs") {
    const example = isHomelabRepo(repoCwd)
      ? "nas/**, proxmox/**, cloudflare/**, dream-machine/**, docs/**, scripts/**, opensrc/**"
      : "packages/**, app/**, docs/**, opensrc/**";
    const value = await ctx.ui.input("Allowed paths", `Comma-separated globs like ${example}`);
    if (!value) throw new Error("setup cancelled");
    const paths = value
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
    if (!paths.length) throw new Error("No allowed paths entered");
    return paths;
  }

  return defaults.allowPaths;
}

function parseFlatYaml(input: string): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const lines = input.split(/\r?\n/);
  let currentKey: string | null = null;
  let inArray = false;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const keyVal = trimmed.match(/^([A-Za-z0-9_\-.]+):\s*(.*)$/);
    if (keyVal) {
      currentKey = keyVal[1];
      const rhs = keyVal[2] ?? "";
      inArray = false;
      if (rhs.startsWith("[")) {
        out[currentKey] = parseInlineArray(rhs);
        currentKey = null;
        continue;
      }
      if (rhs === "") {
        inArray = true;
        out[currentKey] = [];
        continue;
      }
      out[currentKey] = parseScalar(rhs);
      currentKey = null;
      continue;
    }

    if (inArray && currentKey) {
      const item = trimmed.match(/^\-\s*(.*)$/);
      if (item) {
        const existing = Array.isArray(out[currentKey]) ? (out[currentKey] as unknown[]) : [];
        existing.push(parseScalar(item[1] ?? ""));
        out[currentKey] = existing;
      }
    }
  }

  return out;
}

function parseInlineArray(rhs: string): string[] {
  const body = rhs.replace(/^\[/, "").replace(/\]$/, "").trim();
  if (!body) return [];
  return body
    .split(",")
    .map((item) => String(parseScalar(item.trim())))
    .filter((item) => item.length > 0);
}

function parseScalar(value: string): string | boolean | number {
  const unquoted = value.replace(/^['\"]|['\"]$/g, "");
  if (unquoted === "true") return true;
  if (unquoted === "false") return false;
  const numeric = Number(unquoted);
  if (!Number.isNaN(numeric) && unquoted.trim() !== "") return numeric;
  return unquoted;
}

function normalizeSourcePriority(value: unknown): SourcePriority[] | null {
  const items = normalizeStringArray(value);
  if (!items) return null;
  const filtered = items.filter(
    (item): item is SourcePriority => item === "plan" || item === "github" || item === "linear",
  );
  return filtered.length ? filtered : null;
}

function normalizeVerificationProfile(value: unknown): VerificationProfile | null {
  if (value === "conservative" || value === "normal" || value === "strict") return value;
  return null;
}

function normalizeStringArray(value: unknown): string[] | null {
  if (Array.isArray(value)) {
    return value.map((item) => String(item).trim()).filter((item) => item.length > 0);
  }
  if (typeof value === "string" && value.trim()) {
    return value
      .split(",")
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  }
  return null;
}

function normalizeNonEmptyString(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function normalizePositiveInteger(value: unknown): number | null {
  const numeric = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  if (!Number.isFinite(numeric) || numeric <= 0) return null;
  return Math.floor(numeric);
}

function serializeRepoAutopilotConfig(config: RepoAutopilotConfig): string {
  const prefs = {
    enabled: config.enabled ?? false,
    sourcePriority:
      normalizeSourcePriority(config.source_priority) ?? DEFAULT_REPO_AUTOPILOT_PREFERENCES.sourcePriority,
    verificationProfile:
      normalizeVerificationProfile(config.verification_profile) ??
      DEFAULT_REPO_AUTOPILOT_PREFERENCES.verificationProfile,
    allowPaths: ensureOpensrcAllowed(
      normalizeStringArray(config.allow_paths) ?? DEFAULT_REPO_AUTOPILOT_PREFERENCES.allowPaths,
    ),
    denyPaths: stripOpensrcDeny(
      normalizeStringArray(config.deny_paths) ?? DEFAULT_REPO_AUTOPILOT_PREFERENCES.denyPaths,
    ),
    runnerCommandTemplate:
      normalizeNonEmptyString(config.runner_command_template) ??
      DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerCommandTemplate,
    runnerConcurrency:
      normalizePositiveInteger(config.runner_concurrency) ?? DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerConcurrency,
    runnerMaxRepairAttempts:
      normalizePositiveInteger(config.runner_max_repair_attempts) ??
      DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerMaxRepairAttempts,
    runnerIdleTimeoutSeconds:
      normalizePositiveInteger(config.runner_idle_timeout_seconds) ??
      DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerIdleTimeoutSeconds,
    runnerEnvAllowlist:
      normalizeStringArray(config.runner_env_allowlist) ?? DEFAULT_REPO_AUTOPILOT_PREFERENCES.runnerEnvAllowlist,
  };

  return [
    `enabled: ${prefs.enabled ? "true" : "false"}`,
    `source_priority: [${prefs.sourcePriority.join(", ")}]`,
    `verification_profile: ${prefs.verificationProfile}`,
    `allow_paths: [${prefs.allowPaths.join(", ")}]`,
    `deny_paths: [${prefs.denyPaths.join(", ")}]`,
    `runner_command_template: ${JSON.stringify(prefs.runnerCommandTemplate)}`,
    `runner_concurrency: ${prefs.runnerConcurrency}`,
    `runner_max_repair_attempts: ${prefs.runnerMaxRepairAttempts}`,
    `runner_idle_timeout_seconds: ${prefs.runnerIdleTimeoutSeconds}`,
    `runner_env_allowlist: [${prefs.runnerEnvAllowlist.join(", ")}]`,
  ].join("\n");
}

function serializeGlobalAutopilotConfig(allowlist: string[]): string {
  const lines = ["repos:", "  allow:"];
  for (const entry of dedupe(allowlist.map((item) => path.resolve(item)))) {
    lines.push(`    - ${entry}`);
  }
  return lines.join("\n");
}

function dedupe(values: string[]): string[] {
  return [...new Set(values)];
}

function leadingSpaces(line: string): number {
  const match = line.match(/^\s*/);
  return match ? match[0].length : 0;
}
