import { execFileSync, spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import type {
  NativeRunnerConfig,
  RunnerFailureReason,
  WorkerCommandContext,
  WorkerCommandResult,
} from "./v2.ts";
import { buildWorkerCommand, defaultNativeRunnerConfig, nowIso } from "./v2.ts";

export type CommandExecutor = (command: string, options: {
  cwd: string;
  timeoutMs: number;
  logPath?: string;
  env?: NodeJS.ProcessEnv;
}) => Promise<WorkerCommandResult>;

export type NativeRunnerOptions = {
  config?: Partial<NativeRunnerConfig>;
  executor?: CommandExecutor;
};

export type NativeRunner = {
  config: NativeRunnerConfig;
  runWorker(context: WorkerCommandContext, logPath?: string): Promise<WorkerCommandResult>;
};

export type WorktreePreparationResult = {
  worktreePath: string;
  branch: string;
  mode: "git-worktree" | "filesystem-copy";
};

export function createNativeRunner(options: NativeRunnerOptions = {}): NativeRunner {
  const defaults = defaultNativeRunnerConfig();
  const config: NativeRunnerConfig = {
    ...defaults,
    ...options.config,
    concurrency: positiveInteger(options.config?.concurrency, defaults.concurrency),
    maxRepairAttempts: positiveInteger(options.config?.maxRepairAttempts, defaults.maxRepairAttempts),
    idleTimeoutSeconds: positiveInteger(options.config?.idleTimeoutSeconds, defaults.idleTimeoutSeconds),
    envAllowlist: normalizeStringArray(options.config?.envAllowlist) ?? defaults.envAllowlist,
    commandTemplate: options.config?.commandTemplate?.trim() || defaults.commandTemplate,
    evidenceProfile: options.config?.evidenceProfile ?? defaults.evidenceProfile,
  };
  const executor = options.executor ?? executeShellCommand;

  return {
    config,
    runWorker(context, logPath) {
      const command = buildWorkerCommand(config.commandTemplate, context);
      return executor(command, {
        cwd: context.worktreePath,
        timeoutMs: config.idleTimeoutSeconds * 1000,
        logPath,
        env: buildAllowedEnv(process.env, config.envAllowlist),
      });
    },
  };
}

export function prepareFreshIssueWorktree(input: {
  repoCwd: string;
  worktreePath: string;
  branch: string;
}): WorktreePreparationResult {
  fs.mkdirSync(path.dirname(input.worktreePath), { recursive: true });
  if (fs.existsSync(input.worktreePath) && fs.readdirSync(input.worktreePath).length > 0) {
    return { worktreePath: input.worktreePath, branch: input.branch, mode: "filesystem-copy" };
  }
  if (fs.existsSync(input.worktreePath)) fs.rmSync(input.worktreePath, { recursive: true, force: true });

  try {
    execFileSync("git", ["rev-parse", "--verify", "HEAD"], { cwd: input.repoCwd, stdio: "ignore" });
    execFileSync("git", ["worktree", "add", "-B", input.branch, input.worktreePath, "HEAD"], {
      cwd: input.repoCwd,
      stdio: "pipe",
    });
    return { worktreePath: input.worktreePath, branch: input.branch, mode: "git-worktree" };
  } catch {
    copyRepoSnapshot(input.repoCwd, input.worktreePath);
    return { worktreePath: input.worktreePath, branch: input.branch, mode: "filesystem-copy" };
  }
}

function copyRepoSnapshot(source: string, target: string): void {
  fs.mkdirSync(target, { recursive: true });
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    if ([".git", "node_modules"].includes(entry.name)) continue;
    if (entry.name === ".pi") continue;
    const from = path.join(source, entry.name);
    const to = path.join(target, entry.name);
    fs.cpSync(from, to, {
      recursive: true,
      force: true,
      filter: (candidate) => {
        const rel = path.relative(source, candidate).replace(/\\/g, "/");
        return !rel.startsWith("node_modules/") && !rel.startsWith(".git/") && !rel.startsWith(".pi/");
      },
    });
  }
}

function normalizeStringArray(value: unknown): string[] | null {
  if (!Array.isArray(value)) return null;
  const items = value.filter((item): item is string => typeof item === "string").map((item) => item.trim()).filter(Boolean);
  return items.length ? items : null;
}

export function buildAllowedEnv(source: NodeJS.ProcessEnv, allowlist: string[]): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {};
  for (const pattern of allowlist) {
    if (pattern.endsWith("*")) {
      const prefix = pattern.slice(0, -1);
      for (const [key, value] of Object.entries(source)) {
        if (key.startsWith(prefix) && value !== undefined) env[key] = value;
      }
    } else if (source[pattern] !== undefined) {
      env[pattern] = source[pattern];
    }
  }
  return env;
}

function positiveInteger(value: unknown, fallback: number): number {
  const numeric = typeof value === "number" ? value : NaN;
  if (!Number.isFinite(numeric) || numeric <= 0) return fallback;
  return Math.floor(numeric);
}

export async function executeShellCommand(command: string, options: {
  cwd: string;
  timeoutMs: number;
  logPath?: string;
  env?: NodeJS.ProcessEnv;
}): Promise<WorkerCommandResult> {
  const startedAt = nowIso();
  const chunks: string[] = [];
  const errChunks: string[] = [];
  let timeout: NodeJS.Timeout | undefined;
  let timedOut = false;

  if (options.logPath) fs.mkdirSync(path.dirname(options.logPath), { recursive: true });

  const result = await new Promise<{ exitCode: number; failureReason?: RunnerFailureReason }>((resolve) => {
    const child = spawn(command, {
      cwd: options.cwd,
      shell: true,
      stdio: ["ignore", "pipe", "pipe"],
      env: options.env ?? process.env,
    });

    timeout = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, options.timeoutMs);

    child.stdout?.on("data", (data: Buffer) => {
      const text = data.toString();
      chunks.push(text);
      if (options.logPath) fs.appendFileSync(options.logPath, text);
    });

    child.stderr?.on("data", (data: Buffer) => {
      const text = data.toString();
      errChunks.push(text);
      if (options.logPath) fs.appendFileSync(options.logPath, text);
    });

    child.on("error", (error) => {
      errChunks.push(error.message);
      resolve({ exitCode: 1, failureReason: "command-failed" });
    });

    child.on("close", (code) => {
      resolve({
        exitCode: code ?? (timedOut ? 124 : 1),
        failureReason: timedOut ? "timeout" : code === 0 ? undefined : "command-failed",
      });
    });
  });

  if (timeout) clearTimeout(timeout);

  return {
    command,
    cwd: options.cwd,
    startedAt,
    finishedAt: nowIso(),
    exitCode: result.exitCode,
    stdout: chunks.join(""),
    stderr: errChunks.join(""),
    logPath: options.logPath,
    failureReason: result.failureReason,
  };
}
