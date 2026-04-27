import { SessionManager, type ExtensionAPI, type ExtensionCommandContext, type ExtensionContext } from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";

import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import * as crypto from "node:crypto";

import { buildContinueManifest } from "./continue-source.ts";
import { createNativeRunner, prepareFreshIssueWorktree } from "./execution-runner.ts";
import { loadRepoAutopilotConfig, loadRepoAutopilotPreferences, repoAutopilotEnabled, setupAutopilotRepo, setupAutopilotRepoWithOptions, type AutopilotSetupOptions, type SourcePriority, type VerificationProfile } from "./prefs.ts";
import * as v2 from "./v2.ts";

type ManifestFrontmatter = {
  id?: string;
  branch?: string;
  source_kind?: string;
  source_ref?: string;
  source_path?: string;
  source_url?: string;
  source_title?: string;
  source_priority?: string[];
  verification_profile?: string;
  allow_paths?: string[];
  deny_paths?: string[];
  authority?: {
    commit?: boolean;
    push?: boolean;
    pr?: boolean;
    merge?: boolean;
  };
  scope?: {
    gh_issue_ids?: number[];
    linear_issue_ids?: string[];
    todo_ids?: string[];
  };
  paths?: {
    allow?: string[];
    related?: string[];
    deny?: string[];
  };
  checks?: Array<any>;
  verify?: string[];
  acceptance?: string[];
  stop_when?: string[];
  blockers?: any;
  phases?: Array<any>;
  model?: any;
};

type Manifest = {
  file: string;
  frontmatter: ManifestFrontmatter;
  body: string;
};

type LeaseOwner = {
  host: string;
  pid: number;
  startedAt: string;
  heartbeatAt: string;
  runId: string;
  manifestPath: string;
  manifestId?: string;
  sessionFile?: string;
};

type RunLedger = {
  runId: string;
  manifestPath: string;
  manifestId?: string;
  status: "running" | "paused" | "awaiting-user" | "done" | "stopped" | "blocked";
  branch?: string;
  createdAt: string;
  updatedAt: string;
  leaseOwner?: LeaseOwner;
  progress?: {
    failureStreak: number;
    turnCount: number;
    lastOutcome?: string;
    lastPromptHash?: string;
  };
  lastFailingChecks?: Array<any>;
};

type WorktreeIsolationMeta = {
  version: 1;
  originRepoCwd: string;
  originManifestPath: string;
  createdAt: string;
  branchName: string;
  dirtyFiles: string[];
};

const RUN_LEDGER_DIR = "runs";
const LOCKS_DIR = "locks";
const STATUS_DIR = "status";
const LOGS_DIR = "logs";

const AutopilotTransitionParams = Type.Object({
  workflowId: Type.Optional(Type.String({ description: "Workflow id. If omitted, the latest workflow in the current repo is used." })),
  gate: Type.Optional(Type.String({ description: "Approval gate to request: issues, before-issues, execution, or before-execution. Backward-compatible with older calls." })),
  targetPhase: Type.Optional(Type.String({ description: "Workflow phase to request, such as issue-approval, issues-created, execution-approval, ready-to-execute, done, or blocked." })),
  phase: Type.Optional(Type.String({ description: "Alias for targetPhase." })),
  evidencePaths: Type.Optional(Type.Array(Type.String(), { description: "Optional artifact/evidence paths to record on successful phase transitions." })),
  force: Type.Optional(Type.Boolean({ description: "Force a slash-command-style transition override. Use only for explicit human overrides." })),
  note: Type.Optional(Type.String({ description: "Optional approval/transition note or context for the event log." })),
});

function nowIso() {
  return new Date().toISOString();
}

function sleep(ms: number) {
  return new Promise<void>((r) => setTimeout(r, ms));
}

function fileExists(p: string) {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

function readText(p: string) {
  return fs.readFileSync(p, "utf-8");
}

function ensureDir(p: string) {
  fs.mkdirSync(p, { recursive: true });
}

function slugify(input: string) {
  const s = input.trim().toLowerCase();
  return s.replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

function shortHash(input: string) {
  return crypto.createHash("sha256").update(input).digest("hex").slice(0, 16);
}

function shellQuote(value: string) {
  return `'${value.replace(/'/g, `'"'"'`)}'`;
}

function getAutopilotWorktreesRoot() {
  return path.join(os.homedir(), ".pi", "agent", "autopilot", "worktrees");
}

function getIsolationMetaPath(repoCwd: string) {
  return path.join(repoCwd, ".pi", "autopilot", "isolation.json");
}

function readIsolationMeta(repoCwd: string): WorktreeIsolationMeta | null {
  const metaPath = getIsolationMetaPath(repoCwd);
  if (!fileExists(metaPath)) return null;
  try {
    return JSON.parse(readText(metaPath)) as WorktreeIsolationMeta;
  } catch {
    return null;
  }
}

function writeJsonFile(filePath: string, value: unknown) {
  ensureDir(path.dirname(filePath));
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf-8");
}

function copyFileIfExists(sourcePath: string, targetPath: string) {
  if (!fileExists(sourcePath)) return;
  ensureDir(path.dirname(targetPath));
  fs.copyFileSync(sourcePath, targetPath);
}

async function execInDir(
  pi: ExtensionAPI,
  cwd: string,
  command: string,
  timeout = 30_000,
) {
  return pi.exec("bash", ["-lc", `cd ${shellQuote(cwd)} && ${command}`], {
    timeout,
  } as any);
}

async function getGitRoot(pi: ExtensionAPI, repoCwd: string): Promise<string | null> {
  const result = await execInDir(pi, repoCwd, "git rev-parse --show-toplevel", 10_000);
  if (result.code !== 0) return null;
  const root = String(result.stdout ?? "").trim();
  return root ? path.resolve(root) : null;
}

async function getDirtyGitFiles(pi: ExtensionAPI, repoCwd: string): Promise<string[]> {
  const result = await execInDir(pi, repoCwd, "git status --porcelain --untracked-files=all", 15_000);
  if (result.code !== 0) return [];
  return String(result.stdout ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

async function createIsolatedAutopilotSession(args: {
  pi: ExtensionAPI;
  ctx: ExtensionCommandContext;
  repoCwd: string;
  manifestPath: string;
}): Promise<{
  worktreeCwd: string;
  sessionFile: string;
  manifestPath: string;
} | null> {
  const { pi, ctx, repoCwd, manifestPath } = args;

  if (readIsolationMeta(repoCwd)) {
    return null;
  }

  const gitRoot = await getGitRoot(pi, repoCwd);
  if (!gitRoot) return null;

  const dirtyFiles = await getDirtyGitFiles(pi, gitRoot);
  if (!dirtyFiles.length) return null;

  const repoSlug = slugify(path.basename(gitRoot)) || "repo";
  const manifestSlug = slugify(path.basename(manifestPath).replace(/\.md$/i, "")) || "autopilot";
  const repoBucket = `${repoSlug}-${shortHash(path.resolve(gitRoot)).slice(0, 8)}`;
  const worktreeName = `${manifestSlug}-${Date.now().toString(36)}`;
  const worktreeCwd = path.join(getAutopilotWorktreesRoot(), repoBucket, worktreeName);
  const branchName = `autopilot/${repoSlug}-${shortHash(worktreeCwd).slice(0, 8)}`;

  ensureDir(path.dirname(worktreeCwd));
  const addResult = await execInDir(
    pi,
    gitRoot,
    `git worktree add -b ${shellQuote(branchName)} ${shellQuote(worktreeCwd)} HEAD`,
    60_000,
  );
  if (addResult.code !== 0) {
    throw new Error(String(addResult.stderr ?? addResult.stdout ?? "git worktree add failed").trim() || "git worktree add failed");
  }

  const targetManifestPath = path.join(worktreeCwd, ".pi", "autopilot", path.basename(manifestPath));
  ensureDir(path.dirname(targetManifestPath));
  fs.copyFileSync(manifestPath, targetManifestPath);
  copyFileIfExists(path.join(gitRoot, ".pi", "autopilot", "config.yml"), path.join(worktreeCwd, ".pi", "autopilot", "config.yml"));
  copyFileIfExists(path.join(gitRoot, ".pi", "autopilot", "enabled"), path.join(worktreeCwd, ".pi", "autopilot", "enabled"));
  writeJsonFile(getIsolationMetaPath(worktreeCwd), {
    version: 1,
    originRepoCwd: gitRoot,
    originManifestPath: manifestPath,
    createdAt: nowIso(),
    branchName,
    dirtyFiles,
  } satisfies WorktreeIsolationMeta);

  const sessionManager = SessionManager.create(worktreeCwd);
  const sessionFile = sessionManager.getSessionFile();
  if (!sessionFile) {
    throw new Error(`Failed to create session for isolated worktree: ${worktreeCwd}`);
  }

  ctx.ui.notify(
    `Dirty repo detected. Autopilot will run in isolated worktree: ${worktreeCwd}`,
    "info",
  );

  return { worktreeCwd, sessionFile, manifestPath: targetManifestPath };
}

async function maybeStartAutopilotInIsolatedWorktree(args: {
  pi: ExtensionAPI;
  ctx: ExtensionCommandContext;
  repoCwd: string;
  manifestPath: string;
}): Promise<boolean> {
  const isolated = await createIsolatedAutopilotSession(args);
  if (!isolated) return false;

  const manifestArg = path.relative(isolated.worktreeCwd, isolated.manifestPath) || isolated.manifestPath;
  const startCommand = `/autopilot start ${manifestArg}`;

  await args.ctx.switchSession(isolated.sessionFile, {
    withSession: async (replacementCtx) => {
      replacementCtx.ui.notify(
        `Autopilot session switched to isolated worktree on branch ${path.basename(readIsolationMeta(isolated.worktreeCwd)?.branchName ?? "") || "autopilot"}.`,
        "info",
      );
      await replacementCtx.sendUserMessage(startCommand);
    },
  });

  return true;
}

function shouldBlockSensitiveCommand(command: string): string | null {
  const normalized = command.toLowerCase();
  if (/\b(printenv|env)\b/.test(normalized)) {
    return "Refusing to dump environment variables during autopilot.";
  }
  if (/\b(cat|sed|awk|grep|rg)\b[\s\S]*\.env(\.|\b|\s|$)/.test(normalized)) {
    return "Refusing to read .env-style secret files during autopilot.";
  }
  if (/\bbw\s+get\b/.test(normalized) || /\bop\s+item\s+get\b/.test(normalized) || /\bsecurity\s+find-generic-password\b/.test(normalized)) {
    return "Refusing to print secrets from a credential manager during autopilot.";
  }
  if (/mongo[\s\S]*findone\(\{key:\\?"mgmt\\?"\}\)/i.test(command) && !/findone\([\s\S]*\{[\s\S]*(x_ssh_auth_password_enabled|debug_tools_enabled|x_api_token)[\s\S]*\}\)/i.test(command)) {
    return "Refusing to dump full UDM management records. Query only explicitly projected safe fields.";
  }
  return null;
}

function redactSensitiveText(text: string): string {
  return text
    .replace(/((?:x_api_token|x_ssh_password|x_ssh_sha512passwd|x_mgmt_key|api[_-]?token|token|password|passwd|secret|client_secret|authorization|bearer|cookie)\s*["']?\s*[:=]\s*["']?)([^"'\s,}]+)/gi, "$1[REDACTED]")
    .replace(/(mongodb(\+srv)?:\/\/[^\s:/]+:)([^@\s]+)(@)/gi, "$1[REDACTED]$4")
    .replace(/(https?:\/\/[^\s:/]+:)([^@\s]+)(@)/gi, "$1[REDACTED]$3")
    .replace(/(-----begin [a-z0-9 ]*private key-----)[\s\S]*?(-----end [a-z0-9 ]*private key-----)/gi, "$1\n[REDACTED]\n$2")
    .replace(/\b(sk-[A-Za-z0-9_-]{12,}|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{10,})\b/g, "[REDACTED]");
}

function isSensitiveReadPath(filePath: string): boolean {
  const normalized = filePath.replace(/\\/g, "/").toLowerCase();
  return /(^|\/)(\.env($|\.|\/)|\.npmrc$|\.pypirc$|id_rsa$|id_ed25519$|authorized_keys$)/.test(normalized)
    || /(^|\/)(\.ssh|\.gnupg|\.aws)\//.test(normalized)
    || /\.(pem|p12|pfx|key)$/i.test(normalized);
}

function getRepoAutopilotRoot(repoCwd: string) {
  return path.join(repoCwd, ".pi", "autopilot");
}

function getAutopilotDirs(repoCwd: string) {
  const root = getRepoAutopilotRoot(repoCwd);
  return {
    root,
    locksDir: path.join(root, LOCKS_DIR),
    runsDir: path.join(root, RUN_LEDGER_DIR),
    statusDir: path.join(root, STATUS_DIR),
    logsDir: path.join(root, LOGS_DIR),
  };
}

function getGlobalConfigPath() {
  return path.join(os.homedir(), ".pi", "agent", "autopilot", "config.yml");
}

// YAML parsing: try `yaml` first (pi-coding-agent bundles it), then fallback.
function parseYaml(input: string): any {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const yaml = require("yaml");
    if (typeof yaml?.parse !== "function") throw new Error("yaml.parse missing");
    return yaml.parse(input);
  } catch {
    return parseYamlFallback(input);
  }
}

function parseYamlFallback(input: string): any {
  const root: Record<string, any> = {};
  const lines = input.split(/\r?\n/);
  const stack: Array<{ indent: number; container: any; type: "object" | "array" }> = [
    { indent: -1, container: root, type: "object" },
  ];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const indent = leadingIndent(line);
    while (stack.length > 1 && indent <= stack[stack.length - 1]!.indent) {
      stack.pop();
    }

    const parent = stack[stack.length - 1]!;

    if (trimmed.startsWith("- ")) {
      if (parent.type !== "array") {
        throw new Error(`Invalid YAML list item outside array: ${trimmed}`);
      }

      const itemText = trimmed.slice(2).trim();
      const inlineKey = itemText.match(/^([A-Za-z0-9_\-\.]+):\s*(.*)$/);
      if (inlineKey) {
        const item: Record<string, any> = {};
        parent.container.push(item);
        const key = inlineKey[1];
        const rhs = inlineKey[2] ?? "";
        if (rhs === "") {
          item[key] = detectChildContainer(lines, i + 1, indent) === "array" ? [] : {};
        } else {
          item[key] = parseYamlScalar(rhs);
        }
        stack.push({ indent, container: item, type: "object" });
        continue;
      }

      parent.container.push(parseYamlScalar(itemText));
      continue;
    }

    const keyVal = trimmed.match(/^([A-Za-z0-9_\-\.]+):\s*(.*)$/);
    if (!keyVal) continue;

    const key = keyVal[1];
    const rhs = keyVal[2] ?? "";
    if (parent.type !== "object") {
      throw new Error(`Invalid YAML mapping inside array without object wrapper: ${trimmed}`);
    }

    if (rhs === "") {
      const childType = detectChildContainer(lines, i + 1, indent);
      parent.container[key] = childType === "array" ? [] : {};
      stack.push({ indent, container: parent.container[key], type: childType });
      continue;
    }

    parent.container[key] = parseYamlScalar(rhs);
  }

  return root;
}

function detectChildContainer(lines: string[], startIndex: number, parentIndent: number): "object" | "array" {
  for (let i = startIndex; i < lines.length; i++) {
    const line = lines[i] ?? "";
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const indent = leadingIndent(line);
    if (indent <= parentIndent) break;
    return trimmed.startsWith("- ") ? "array" : "object";
  }
  return "object";
}

function parseYamlScalar(value: string): any {
  const trimmed = value.trim();
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    const body = trimmed.slice(1, -1).trim();
    return body ? body.split(",").map((part) => parseYamlScalar(part)) : [];
  }

  const unquoted = trimmed.replace(/^['\"]|['\"]$/g, "");
  if (unquoted === "true") return true;
  if (unquoted === "false") return false;
  const num = Number(unquoted);
  if (!Number.isNaN(num) && unquoted !== "") return num;
  return unquoted;
}

function leadingIndent(line: string): number {
  return line.match(/^\s*/)?.[0].length ?? 0;
}

function parseManifest(filePath: string): Manifest {
  const absPath = path.isAbsolute(filePath)
    ? filePath
    : path.resolve(process.cwd(), filePath);

  if (!fileExists(absPath)) {
    throw new Error(`Manifest not found: ${absPath}`);
  }

  const raw = readText(absPath);
  const fmMatch = raw.match(/^---\s*\n([\s\S]*?)\n---\s*\n?/);
  if (!fmMatch) {
    throw new Error(
      `Manifest must start with YAML frontmatter delimited by ---: ${absPath}`,
    );
  }

  const fmRaw = fmMatch[1];
  const body = raw.slice(fmMatch[0].length);

  const frontmatter = (parseYaml(fmRaw) ?? {}) as ManifestFrontmatter;
  return { file: absPath, frontmatter, body };
}

function looksLikeAutopilotManifest(raw: string): boolean {
  const fmMatch = raw.match(/^---\s*\n([\s\S]*?)\n---\s*\n?/);
  if (!fmMatch) {
    return false;
  }

  return /(^|\n)(id|authority|scope|paths|checks|verify|acceptance|stop_when):/m.test(fmMatch[1] ?? "");
}

async function resolveStartManifestPath(pi: ExtensionAPI, repoCwd: string, input: string): Promise<{ manifestPath: string; mode: "manifest" | "generated" }> {
  const normalized = input.trim();
  if (!normalized) {
    throw new Error("missing source or manifest path");
  }

  const absolute = path.isAbsolute(normalized)
    ? normalized
    : path.resolve(repoCwd, normalized);

  if (fileExists(absolute)) {
    const raw = readText(absolute);
    if (looksLikeAutopilotManifest(raw)) {
      return { manifestPath: absolute, mode: "manifest" };
    }

    const { manifestPath } = await buildContinueManifest(pi, repoCwd, absolute);
    return { manifestPath, mode: "generated" };
  }

  const { manifestPath } = await buildContinueManifest(pi, repoCwd, normalized);
  return { manifestPath, mode: "generated" };
}

function buildAutopilotPlanPrompt(args: { repoCwd: string; topic: string; outputPath: string }) {
  const { repoCwd, topic, outputPath } = args;
  const grillMeSkillPath = path.join(os.homedir(), ".pi", "agent", "skills", "grill-me", "SKILL.md");
  const theFoolSkillPath = path.join(os.homedir(), ".pi", "agent", "skills", "the-fool", "SKILL.md");

  return [
    "I want an autopilot-ready implementation spec for this repo.",
    `Repo root: ${repoCwd}`,
    `Write the final spec to: ${outputPath}`,
    "",
    "Before doing anything else:",
    `1. Read this skill completely: ${grillMeSkillPath}`,
    `2. Read this skill completely: ${theFoolSkillPath}`,
    "",
    "Workflow:",
    "- Start with the grill-me workflow. Interview me one question at a time until the plan is concrete and unambiguous. Use AskUserQuestion for decision points.",
    "- After the questioning phase is complete, apply the-fool as a pre-mortem / strongest-challenges pass to stress-test the plan.",
    "- Then synthesize a final implementation spec that is ready to use as input to `/autopilot start <planPath>`.",
    "- The final spec should be plain markdown, not an autopilot manifest.",
    "",
    "The final spec must include:",
    "- Title",
    "- Goal",
    "- Background / problem statement",
    "- Constraints and non-goals",
    "- Files, systems, or services likely affected",
    "- Acceptance criteria",
    "- Verification / checks",
    "- Step-by-step implementation plan",
    "- Rollback or safety notes when relevant",
    "- Open questions only if they are truly unresolved",
    "",
    "Do not write the final spec until the questioning and challenge passes are complete.",
    "",
    `Planning topic: ${topic}`,
  ].join("\n");
}

function getGlobalConfig(repoCwd: string) {
  const configPath = getGlobalConfigPath();
  if (!fileExists(configPath)) return {};
  try {
    return parseYaml(readText(configPath)) ?? {};
  } catch {
    return {};
  }
}

function getRepoConfig(repoCwd: string) {
  const repoConfigPath = path.join(repoCwd, ".pi", "autopilot", "config.yml");
  if (!fileExists(repoConfigPath)) return {};
  try {
    return parseYaml(readText(repoConfigPath)) ?? {};
  } catch {
    return {};
  }
}

function repoIsOptedIn(repoCwd: string) {
  return repoAutopilotEnabled(repoCwd);
}

function checkRepoAllowlisted(pi: ExtensionAPI, repoCwd: string): boolean {
  const isolation = readIsolationMeta(repoCwd);
  if (isolation) {
    return checkRepoAllowlisted(pi, isolation.originRepoCwd);
  }

  // v2 policy: no double opt-in. A repo is trusted if it has a local marker/config OR
  // appears in the global allowlist. Commands auto-create the local marker on first use.
  if (repoIsOptedIn(repoCwd)) return true;

  const globalConfig = getGlobalConfig(repoCwd);
  const repoRoot = repoCwd;
  const allowed: string[] =
    (globalConfig?.repos?.allow ?? []).map((p: string) => p.trim()) || [];

  return allowed.some((p) => {
    const abs = path.isAbsolute(p) ? p : path.resolve(repoRoot, p);
    return path.resolve(abs) === path.resolve(repoRoot);
  });
}

function lockPathForSlug(repoCwd: string, slug: string) {
  const { locksDir } = getAutopilotDirs(repoCwd);
  return path.join(locksDir, `${slug}.json`);
}

function runLedgerPathForRunId(repoCwd: string, runId: string) {
  const { runsDir } = getAutopilotDirs(repoCwd);
  return path.join(runsDir, `${runId}.json`);
}

function loadRunLedger(repoCwd: string, runId: string): RunLedger | null {
  const p = runLedgerPathForRunId(repoCwd, runId);
  if (!fileExists(p)) return null;
  try {
    return JSON.parse(readText(p)) as RunLedger;
  } catch {
    return null;
  }
}

function saveRunLedger(repoCwd: string, ledger: RunLedger) {
  const p = runLedgerPathForRunId(repoCwd, ledger.runId);
  fs.writeFileSync(p, JSON.stringify(ledger, null, 2), "utf-8");
}

function loadLeaseOwner(repoCwd: string, slug: string): LeaseOwner | null {
  const lp = lockPathForSlug(repoCwd, slug);
  if (!fileExists(lp)) return null;
  try {
    return JSON.parse(readText(lp)) as LeaseOwner;
  } catch {
    return null;
  }
}

function saveLeaseOwner(repoCwd: string, slug: string, owner: LeaseOwner) {
  const { locksDir } = getAutopilotDirs(repoCwd);
  ensureDir(locksDir);
  const lp = lockPathForSlug(repoCwd, slug);
  fs.writeFileSync(lp, JSON.stringify(owner, null, 2), "utf-8");
}

function isLeaseStale(owner: LeaseOwner | null, staleAfterMs: number) {
  if (!owner) return false;
  const last = Date.parse(owner.heartbeatAt);
  if (Number.isNaN(last)) return true;
  return Date.now() - last > staleAfterMs;
}

function parseSetupCommandArgs(raw: string): { interactive: boolean; help: boolean; options: AutopilotSetupOptions } {
  const tokens = tokenizeArgs(raw);
  if (!tokens.length) {
    return {
      interactive: true,
      help: false,
      options: {
        enabled: true,
        sourcePriority: ["plan", "github", "linear"],
        verificationProfile: "normal",
        allowPaths: ["packages/**", "app/**", "docs/**", "opensrc/**"],
        denyPaths: ["node_modules/**"],
      },
    };
  }

  if (tokens.some((token) => token === "--help" || token === "-h")) {
    return {
      interactive: false,
      help: true,
      options: {},
    };
  }

  if (tokens.some((token) => token === "--interactive" || token === "interactive")) {
    return {
      interactive: true,
      help: false,
      options: {},
    };
  }

  const positional: string[] = [];
  const parsed: AutopilotSetupOptions = { enabled: true };

  for (let i = 0; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (!token) continue;

    if (token.startsWith("--")) {
      const [flag, inlineValue] = token.split("=", 2);
      const value = inlineValue ?? tokens[i + 1];
      switch (flag) {
        case "--source":
          if (!value || value.startsWith("--")) throw new Error("--source needs a value");
          parsed.sourcePriority = parseSourcePriority(value);
          if (!inlineValue) i += 1;
          break;
        case "--verify":
        case "--verification":
          if (!value || value.startsWith("--")) throw new Error("--verify needs a value");
          parsed.verificationProfile = parseVerificationProfile(value);
          if (!inlineValue) i += 1;
          break;
        case "--allow":
          if (!value || value.startsWith("--")) throw new Error("--allow needs a value");
          parsed.allowPaths = parseCsvGlobs(value);
          if (!inlineValue) i += 1;
          break;
        case "--deny":
          if (!value || value.startsWith("--")) throw new Error("--deny needs a value");
          parsed.denyPaths = parseCsvGlobs(value);
          if (!inlineValue) i += 1;
          break;
        case "--enabled":
          parsed.enabled = true;
          break;
        case "--disabled":
        case "--no-enabled":
          parsed.enabled = false;
          break;
        default:
          throw new Error(`Unknown setup flag: ${flag}`);
      }
      continue;
    }

    positional.push(token);
  }

  if (positional[0]) {
    parsed.sourcePriority = parseSourcePriority(positional[0]);
  }
  if (positional[1]) {
    parsed.verificationProfile = parseVerificationProfile(positional[1]);
  }
  if (positional[2]) {
    parsed.allowPaths = parseCsvGlobs(positional[2]);
  }
  if (positional[3]) {
    parsed.denyPaths = parseCsvGlobs(positional[3]);
  }

  return {
    interactive: false,
    help: false,
    options: parsed,
  };
}

function tokenizeArgs(input: string): string[] {
  const tokens: string[] = [];
  const pattern = /"([^"]*)"|'([^']*)'|(\S+)/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(input)) !== null) {
    tokens.push(match[1] ?? match[2] ?? match[3] ?? "");
  }
  return tokens;
}

function parseSourcePriority(value: string): SourcePriority[] {
  const normalized = value.trim().toLowerCase();
  if (normalized === "github" || normalized === "gh" || normalized === "github-first") return ["github", "linear", "plan"];
  if (normalized === "linear" || normalized === "linear-first") return ["linear", "github", "plan"];
  if (normalized === "mixed" || normalized === "any" || normalized === "all" || normalized === "plan") return ["plan", "github", "linear"];
  throw new Error(`Unknown source preference: ${value}`);
}

function parseVerificationProfile(value: string): VerificationProfile {
  const normalized = value.trim().toLowerCase();
  if (normalized === "conservative") return "conservative";
  if (normalized === "normal") return "normal";
  if (normalized === "strict") return "strict";
  throw new Error(`Unknown verification profile: ${value}`);
}

function parseCsvGlobs(value: string): string[] {
  const paths = value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
  if (!paths.length) {
    throw new Error(`Empty path list: ${value}`);
  }
  return paths;
}

function buildNextPrompt(args: {
  manifest: Manifest;
  failingChecks: Array<any>;
  iteration: number;
}): string {
  const { manifest, failingChecks, iteration } = args;

  const checksLabel = failingChecks.length
    ? "Failing checks"
    : "No failing checks";

  const checksText = failingChecks.length
    ? failingChecks
        .map((c) => {
          const suffix = c?.reason ? ` (${c.reason})` : "";
          if (typeof c === "string") return `- ${c}${suffix}`;
          if (c?.id) return `- ${c.id}${suffix}`;
          if (c?.type === "command") return `- command: ${c.run ?? c.command ?? ""}${suffix}`;
          if (c?.type === "verify") return `- verify: ${c.check ?? c.id ?? ""}${suffix}`;
          return `- ${JSON.stringify(c)}${suffix}`;
        })
        .join("\n")
    : "(none)";

  const base = [
    `Autopilot run continues (iteration ${iteration}).`,
    `Manifest: ${manifest.file}`,
    `${checksLabel}:\n${checksText}`,
    "",
    "Hard rules:",
    "- Only do work needed to make verification pass.",
    "- If stuck, report what failed and what you need from me.",
    "- Keep changes inside allowed paths; never touch denied paths.",
    "- Never print or persist secrets, passwords, tokens, hashes, or full secret-bearing command output.",
    "- Only mutate remote systems or GitHub state when the manifest/source explicitly requires it.",
    "",
    "Stop condition:",
    "- Once all required checks pass, reply 'AUTOPILOT DONE'.",
  ].join("\n");

  return base;
}

async function runCommandChecks(
  pi: ExtensionAPI,
  ctx: ExtensionContext,
  check: any,
): Promise<{ pass: boolean; reason?: string }> {
  // Supported shapes:
  // - { type: 'command', run: 'bun run typecheck' }
  // - { type: 'rg', path: '...', pattern: '...' }
  // - { type: 'file_exists', path: '...' }
  // - string command
  try {
    if (typeof check === "string") {
      const { code } = await pi.exec("bash", ["-lc", check], {
        timeout: 10 * 60 * 1000,
      } as any);
      return { pass: code === 0, reason: code === 0 ? undefined : `exit ${code}` };
    }

    const t = check?.type;
    if (!t) {
      // treat as raw command if `run` or `command`
      const cmd = check?.run ?? check?.command;
      if (!cmd || typeof cmd !== "string") {
        return { pass: false, reason: `Unknown check shape: ${JSON.stringify(check)}` };
      }
      const { code } = await pi.exec("bash", ["-lc", cmd], {
        timeout: 10 * 60 * 1000,
      } as any);
      return { pass: code === 0, reason: code === 0 ? undefined : `exit ${code}` };
    }

    if (t === "command") {
      const cmd = check.run;
      if (!cmd || typeof cmd !== "string") {
        return { pass: false, reason: "command.check missing run" };
      }
      const { code } = await pi.exec("bash", ["-lc", cmd], {
        timeout: 10 * 60 * 1000,
      } as any);
      return { pass: code === 0, reason: code === 0 ? undefined : `exit ${code}` };
    }

    if (t === "file_exists") {
      const p = check.path;
      if (!p || typeof p !== "string") return { pass: false, reason: "file_exists missing path" };
      const abs = path.isAbsolute(p) ? p : path.join(ctx.cwd, p);
      return { pass: fileExists(abs), reason: fileExists(abs) ? undefined : `missing ${abs}` };
    }

    if (t === "rg") {
      const p = check.path;
      const pattern = check.pattern;
      if (!p || !pattern) return { pass: false, reason: "rg missing path or pattern" };
      const target = path.isAbsolute(p) ? p : path.join(ctx.cwd, p);
      const { code } = await pi.exec(
        "rg",
        ["-n", pattern, target],
        {
          timeout: 60 * 1000,
        } as any,
      );
      // rg returns 1 on no matches
      return { pass: code === 0, reason: code === 0 ? undefined : "no matches" };
    }

    return { pass: false, reason: `Unsupported check type: ${t}` };
  } catch (e) {
    return { pass: false, reason: e instanceof Error ? e.message : String(e) };
  }
}

function normalizeManifestChecks(raw: unknown): any[] {
  if (!Array.isArray(raw)) return [];
  const normalized: any[] = [];

  for (let i = 0; i < raw.length; i += 1) {
    const item = raw[i];
    if (typeof item === "string") {
      const trimmed = item.trim();
      if (!trimmed || trimmed === "all_required_checks_passed") continue;

      // Recovery for broken fallback YAML parses that flatten:
      // checks:\n  - type: command\n    run: bun test
      if (/^type:\s*command$/i.test(trimmed)) {
        const next = typeof raw[i + 1] === "string" ? raw[i + 1].trim() : "";
        const run = next.match(/^run:\s*(.+)$/i)?.[1]?.trim();
        if (run) {
          normalized.push({ type: "command", run });
          i += 1;
          continue;
        }
        continue;
      }

      const run = trimmed.match(/^run:\s*(.+)$/i)?.[1]?.trim();
      if (run) {
        normalized.push({ type: "command", run });
        continue;
      }

      normalized.push(trimmed);
      continue;
    }

    if (item && typeof item === "object") {
      const record = item as Record<string, any>;
      if (record.type === "command" && typeof record.run === "string" && record.run.trim()) {
        normalized.push({ ...record, run: record.run.trim() });
        continue;
      }
      if (record.type === "rg" || record.type === "file_exists") {
        normalized.push(record);
        continue;
      }
      if (!record.type && typeof record.run === "string") {
        normalized.push({ ...record, type: "command", run: record.run.trim() });
        continue;
      }
    }
  }

  return normalized;
}

function normalizeManifestVerify(raw: unknown): any[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => typeof item === "string" ? item.trim() : item)
    .filter((item) => item !== "" && item !== "all_required_checks_passed");
}

async function evaluateChecks(
  pi: ExtensionAPI,
  ctx: ExtensionContext,
  manifest: Manifest,
  failingSoFar: Array<any>,
): Promise<{ failing: Array<any>; allPass: boolean }> {
  const checkList = normalizeManifestChecks(manifest.frontmatter.checks);
  const verifyList = normalizeManifestVerify(manifest.frontmatter.verify);

  const failing: Array<any> = [];
  for (const c of checkList) {
    const res = await runCommandChecks(pi, ctx, c);
    if (!res.pass) {
      failing.push({ ...((typeof c === "object" && c) ? c : { type: "command", run: c }), reason: res.reason });
    }
  }

  for (const v of verifyList) {
    const res = await runCommandChecks(pi, ctx, v);
    if (!res.pass) {
      failing.push({ ...((typeof v === "object" && v) ? v : { type: "verify", check: v }), reason: res.reason });
    }
  }

  return { failing, allPass: failing.length === 0 };
}

async function writeStatusWidget(repoCwd: string, ui: ExtensionContext["ui"], ledger: RunLedger, failing: Array<any>) {
  // Local-only notification: update status line + small widget if UI exists.
  const statusLine = `autopilot:${ledger.status} run:${ledger.runId.slice(0, 8)}`;
  ui.setStatus("autopilot", statusLine);

  const lines: string[] = [];
  lines.push(`manifest:${ledger.manifestId ?? ledger.manifestPath}`);
  lines.push(`status:${ledger.status}`);
  if (failing.length) {
    lines.push(`failing:${failing.length}`);
    for (const f of failing.slice(0, 6)) {
      const label = typeof f === "string" ? f : f?.id ?? f?.type ?? "check";
      lines.push(`- ${label}`);
    }
    if (failing.length > 6) lines.push(`... +${failing.length - 6} more`);
  } else {
    lines.push("checks:pass");
  }

  ui.setWidget("autopilot-widget", lines);
}

function sendAutopilotPrompt(pi: ExtensionAPI, ctx: ExtensionContext, prompt: string) {
  if (ctx.isIdle()) {
    pi.sendUserMessage(prompt);
  } else {
    pi.sendUserMessage(prompt, { deliverAs: "followUp" });
  }
}

function createAndLaunchV2Workflow(args: {
  pi: ExtensionAPI;
  ctx: ExtensionCommandContext;
  repoCwd: string;
  lane: v2.WorkflowLane;
  input: string;
}) {
  const workflow = v2.createWorkflow({
    repoCwd: args.repoCwd,
    lane: args.lane,
    rawInput: args.input,
  });

  const prompt = args.lane === "architecture"
    ? v2.buildArchitecturePrompt(workflow)
    : v2.buildPlanningPrompt(workflow);

  sendAutopilotPrompt(args.pi, args.ctx, prompt);
  args.ctx.ui.notify(
    `Autopilot v2 ${args.lane} workflow created: ${workflow.workflowId} (${workflow.paths.workflowDir})`,
    "info",
  );
}

function resolveWorkflowOrNotify(ctx: ExtensionContext, repoCwd: string, selector: string): v2.WorkflowState | null {
  const workflows = v2.listWorkflowStates(repoCwd);
  if (!selector.trim()) {
    if (workflows.length === 1) return workflows[0]!;
    if (workflows.length > 1) {
      ctx.ui.notify(`Multiple workflows found. Use /autopilot workflows, then pass a workflow id.`, "warning");
      return null;
    }
    ctx.ui.notify("No v2 workflows found. Use /autopilot plan <idea> first.", "warning");
    return null;
  }

  const workflow = v2.findWorkflowState(repoCwd, selector.trim());
  if (!workflow) {
    ctx.ui.notify(`Workflow not found: ${selector}`, "error");
    return null;
  }
  return workflow;
}

async function ensureAutopilotReadyForRepo(ctx: ExtensionCommandContext, repoCwd: string): Promise<boolean> {
  if (checkRepoAllowlisted({} as ExtensionAPI, repoCwd)) return true;

  try {
    const result = await setupAutopilotRepoWithOptions(repoCwd, getGlobalConfigPath(), {
      enabled: true,
      sourcePriority: ["plan", "github", "linear"],
      verificationProfile: "normal",
    });
    v2.ensureRepoV2Scaffold(repoCwd);
    ctx.ui.notify(
      `Autopilot auto-configured for ${repoCwd}. Profile=${result.preferences.verificationProfile}, sources=${result.preferences.sourcePriority.join(">")}.`,
      "info",
    );
    return true;
  } catch (error) {
    ctx.ui.notify(
      `Autopilot auto-configuration failed for ${repoCwd}: ${error instanceof Error ? error.message : String(error)}`,
      "error",
    );
    return false;
  }
}

async function resolveV2RepoCwd(pi: ExtensionAPI, cwd: string): Promise<string> {
  const gitRoot = await getGitRoot(pi, cwd);
  return gitRoot ?? cwd;
}

function newestPreArtifactPlanningWorkflow(repoCwd: string): v2.WorkflowState | null {
  return v2.listWorkflowStates(repoCwd).find((workflow) =>
    workflow.lane === "planning" && v2.isPreArtifactPlanningPhase(workflow.phase)
  ) ?? null;
}

function lockedPlanningArtifactWriteReason(repoCwd: string, rawPath: string): string | null {
  const workflow = newestPreArtifactPlanningWorkflow(repoCwd);
  if (!workflow) return null;
  const result = v2.isPlanningArtifactLockedPath(workflow, rawPath);
  return result.locked ? result.reason ?? "Autopilot planning artifacts are locked until concept lock." : null;
}

function lockedPlanningArtifactCommandReason(repoCwd: string, command: string): string | null {
  const workflow = newestPreArtifactPlanningWorkflow(repoCwd);
  if (!workflow) return null;
  for (const artifactPath of v2.lockedPlanningArtifactPaths(workflow)) {
    const relative = path.relative(repoCwd, artifactPath);
    if (command.includes(artifactPath) || command.includes(relative)) {
      if (/\b(tee|touch|rm|mv|cp|mkdir|python\d*|node|cat)\b|>>?|\bwriteFileSync\b|\bwriteFile\b/.test(command)) {
        return `Autopilot planning is in ${workflow.phase}; artifact mutation is locked until concept lock: ${relative}`;
      }
    }
  }
  return null;
}

function parseApprovalGateToken(gateToken: string): v2.ApprovalGate | null {
  if (["issues", "issue", "before-issues", "before_issues"].includes(gateToken)) return "before-issues";
  if (["execution", "exec", "before-execution", "before_execution"].includes(gateToken)) return "before-execution";
  return null;
}

async function approveWorkflowGateWithUi(args: {
  pi: ExtensionAPI;
  ctx: ExtensionContext;
  repoCwd: string;
  gate: v2.ApprovalGate;
  selector: string;
  note?: string;
}): Promise<v2.WorkflowState | null> {
  const workflow = resolveWorkflowOrNotify(args.ctx, args.repoCwd, args.selector);
  if (!workflow) return null;

  const ok = await args.ctx.ui.confirm(
    `Approve ${args.gate}?`,
    `Workflow ${workflow.workflowId}\nThis persists a durable approval and may allow the next phase to create tracker issues or execute code.`,
  );
  if (!ok) {
    args.ctx.ui.notify("Approval cancelled.", "warning");
    return null;
  }

  const approvedBy = (() => {
    try {
      return os.userInfo().username;
    } catch {
      return "ossie";
    }
  })();
  const approved = v2.approveGate(workflow, args.gate, approvedBy, args.note);
  sendAutopilotPrompt(args.pi, args.ctx, v2.buildApprovalPrompt(approved, args.gate));
  args.ctx.ui.notify(`Approved ${args.gate} for ${approved.workflowId}.`, "info");
  return approved;
}

async function approveV2WorkflowGate(args: {
  pi: ExtensionAPI;
  ctx: ExtensionCommandContext;
  repoCwd: string;
  rest: string;
}) {
  const tokens = tokenizeArgs(args.rest);
  const gateToken = tokens[0] ?? "";
  const selector = tokens[1] ?? "";
  const note = tokens.slice(2).join(" ").trim() || undefined;
  const gate = parseApprovalGateToken(gateToken);

  if (!gate) {
    args.ctx.ui.notify("Usage: /autopilot approve <issues|execution> <workflow-id> [note]", "error");
    return;
  }

  await approveWorkflowGateWithUi({
    pi: args.pi,
    ctx: args.ctx,
    repoCwd: args.repoCwd,
    gate,
    selector,
    note,
  });
}

async function transitionV2WorkflowPhase(args: {
  ctx: ExtensionCommandContext;
  repoCwd: string;
  rest: string;
}) {
  const tokens = tokenizeArgs(args.rest);
  const selector = tokens[0] ?? "";
  const targetPhaseToken = tokens[1] ?? "";
  const force = tokens.includes("--force");
  const note = tokens.slice(2).filter((token) => token !== "--force").join(" ").trim() || undefined;

  if (!selector || !targetPhaseToken || !v2.isWorkflowPhase(targetPhaseToken)) {
    args.ctx.ui.notify("Usage: /autopilot transition <workflow-id> <phase> [--force] [note]", "error");
    return;
  }

  const workflow = resolveWorkflowOrNotify(args.ctx, args.repoCwd, selector);
  if (!workflow) return;

  const validation = v2.validateWorkflowTransition(workflow, targetPhaseToken, { force });
  if (!validation.ok && !force) {
    args.ctx.ui.notify(`Transition rejected: ${validation.reasons.join(" ")}`, "warning");
    return;
  }

  const ok = await args.ctx.ui.confirm(
    force ? `Force transition to ${targetPhaseToken}?` : `Transition to ${targetPhaseToken}?`,
    `Workflow ${workflow.workflowId}\nCurrent phase: ${workflow.phase}\nTarget phase: ${targetPhaseToken}`,
  );
  if (!ok) {
    args.ctx.ui.notify("Transition cancelled.", "warning");
    return;
  }

  const result = v2.transitionWorkflowPhase(workflow, targetPhaseToken, {
    actor: "slash-command",
    note,
    force,
  });

  if (!result.ok) {
    args.ctx.ui.notify(`Transition rejected: ${result.validation.reasons.join(" ")}`, "warning");
    return;
  }

  args.ctx.ui.notify(`Workflow ${workflow.workflowId} transitioned to ${targetPhaseToken}.`, "info");
}

function normalizeAutopilotCommandArgs(raw: string): string {
  let text = raw.trim();

  const fenced = text.match(/^```(?:bash|sh|shell|txt)?\s*\n([\s\S]*?)\n?```$/i);
  if (fenced) {
    text = (fenced[1] ?? "").trim();
  }

  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const slashCommandLine = lines.find((line) => /^\$?\s*\/?autopilot(?:\s|$)/.test(line));
  if (slashCommandLine) {
    text = slashCommandLine.trim();
  }

  text = text.replace(/^\$\s*/, "").trim();
  text = text.replace(/^\/?autopilot(?:\s+|$)/, "").trim();
  return text;
}

function buildStatusSummary(repoCwd: string, allowed: boolean) {
  const root = getRepoAutopilotRoot(repoCwd);
  const { runsDir, locksDir } = getAutopilotDirs(repoCwd);
  const lockFiles = fileExists(locksDir)
    ? fs.readdirSync(locksDir).filter((f) => f.endsWith(".json"))
    : [];
  const runFiles = fileExists(runsDir)
    ? fs.readdirSync(runsDir).filter((f) => f.endsWith(".json"))
    : [];
  const repoConfig = loadRepoAutopilotConfig(repoCwd);
  const prefs = loadRepoAutopilotPreferences(repoCwd);
  const workflows = v2.listWorkflowStates(repoCwd);
  const latest = workflows[0];

  const summary = {
    root,
    allowed,
    enabled: repoAutopilotEnabled(repoCwd),
    verificationProfile: repoConfig.verification_profile ?? prefs.verificationProfile,
    sourcePriority: prefs.sourcePriority,
    v1: { locks: lockFiles.length, runs: runFiles.length },
    v2: {
      workflows: workflows.length,
      latest: latest
        ? {
            workflowId: latest.workflowId,
            lane: latest.lane,
            status: latest.status,
            phase: latest.phase,
            source: latest.source.title,
          }
        : null,
    },
  };
  v2.writeLatestStatus(repoCwd, summary);

  const latestText = latest
    ? `, latest=${latest.workflowId} ${latest.status}/${latest.phase}`
    : "";

  return `autopilot root: ${root} (allowed=${allowed ? "yes" : "no"}, enabled=${summary.enabled ? "yes" : "no"}, ` +
    `v1 locks=${lockFiles.length}, v1 runs=${runFiles.length}, v2 workflows=${workflows.length}${latestText}, ` +
    `verify=${summary.verificationProfile}, sources=${summary.sourcePriority.join(">")})` +
    `${allowed ? "" : " — run /autopilot setup to enable this repo."}`;
}

export default function (pi: ExtensionAPI) {
  let heartbeatTimer: NodeJS.Timeout | null = null;
  let armed = false;
  let activeRepoCwd: string | null = null;
  let activeSlug: string | null = null;
  let activeRunId: string | null = null;
  let manifest: Manifest | null = null;
  let lastFailingChecks: Array<any> = [];
  let failureStreak = 0;
  let turnCount = 0;

  function stopHeartbeat() {
    if (heartbeatTimer) clearInterval(heartbeatTimer);
    heartbeatTimer = null;
  }

  function startHeartbeat(repoCwd: string, slug: string, owner: LeaseOwner) {
    stopHeartbeat();
    const heartbeatMs = 20_000;
    heartbeatTimer = setInterval(() => {
      try {
        owner.heartbeatAt = nowIso();
        saveLeaseOwner(repoCwd, slug, owner);
      } catch {
        // ignore
      }
    }, heartbeatMs);
  }

  async function acquireOrTakeLease(opts: {
    ctx: ExtensionContext;
    repoCwd: string;
    slug: string;
    runId: string;
    manifestPath: string;
    manifestId?: string;
    force?: boolean;
  }): Promise<{ ok: boolean; leaseOwner?: LeaseOwner; reason?: string }> {
    const { ctx, repoCwd, slug, runId, manifestPath, manifestId, force } = opts;
    ensureDir(getRepoAutopilotRoot(repoCwd));
    const { locksDir, runsDir, statusDir, logsDir } = getAutopilotDirs(repoCwd);
    ensureDir(locksDir);
    ensureDir(runsDir);
    ensureDir(statusDir);
    ensureDir(logsDir);

    const staleAfterMs = 120_000;
    const currentOwner = loadLeaseOwner(repoCwd, slug);

    if (currentOwner && !isLeaseStale(currentOwner, staleAfterMs)) {
      if (!force) {
        return { ok: false, reason: `Lease active (owner pid=${currentOwner.pid} host=${currentOwner.host})` };
      }
    }

    const owner: LeaseOwner = {
      host: os.hostname(),
      pid: process.pid,
      startedAt: nowIso(),
      heartbeatAt: nowIso(),
      runId,
      manifestPath,
      manifestId,
      sessionFile: ctx.sessionManager.getSessionFile() ?? undefined,
    };

    saveLeaseOwner(repoCwd, slug, owner);
    return { ok: true, leaseOwner: owner };
  }

  function updateInMemoryAfterLoad(repoCwd: string, slug: string, runId: string, m: Manifest, ledger: RunLedger) {
    activeRepoCwd = repoCwd;
    activeSlug = slug;
    activeRunId = runId;
    manifest = m;
    failureStreak = ledger.progress?.failureStreak ?? 0;
    turnCount = ledger.progress?.turnCount ?? 0;
    lastFailingChecks = ledger.lastFailingChecks ?? [];
    armed = ledger.status === "running" || ledger.status === "paused" || ledger.status === "awaiting-user" || ledger.status === "blocked";
  }

  async function stopRun(repoCwd: string) {
    if (!activeRunId) return;
    const ledger = loadRunLedger(repoCwd, activeRunId);
    if (!ledger) return;

    ledger.status = "stopped";
    ledger.updatedAt = nowIso();
    ledger.progress = {
      ...(ledger.progress ?? { failureStreak: 0, turnCount: 0 }),
      lastOutcome: "stopped",
    };

    saveRunLedger(repoCwd, ledger);
    stopHeartbeat();
    armed = false;
  }

  async function markStatus(repoCwd: string, runId: string, status: RunLedger["status"], ctx?: ExtensionContext, failingChecks?: Array<any>) {
    const ledger = loadRunLedger(repoCwd, runId);
    if (!ledger) return;
    ledger.status = status;
    ledger.updatedAt = nowIso();
    if (failingChecks) ledger.lastFailingChecks = failingChecks;
    saveRunLedger(repoCwd, ledger);

    if (ctx) {
      await writeStatusWidget(repoCwd, ctx.ui, ledger, failingChecks ?? []);
    }
  }

  async function autopilotTick(ctx: ExtensionContext) {
    if (!armed || !activeRepoCwd || !activeSlug || !activeRunId || !manifest) return;
    const ledger = loadRunLedger(activeRepoCwd, activeRunId);
    if (!ledger) return;

    // Only tick while lease is still owned.
    const owner = loadLeaseOwner(activeRepoCwd, activeSlug);
    if (!owner || owner.runId !== activeRunId) {
      armed = false;
      await markStatus(activeRepoCwd, activeRunId, "blocked", ctx, lastFailingChecks);
      ctx.ui.notify("Autopilot lease lost. Pausing.", "warning");
      return;
    }

    if (["paused", "awaiting-user"].includes(ledger.status)) return;
    if (ledger.status !== "running" && ledger.status !== "blocked") return;

    const evaluation = await evaluateChecks(pi, ctx, manifest, lastFailingChecks);
    lastFailingChecks = evaluation.failing;

    // Update ledger progress
    ledger.progress = {
      ...(ledger.progress ?? { failureStreak: 0, turnCount: 0 }),
      failureStreak: evaluation.allPass ? 0 : (ledger.progress?.failureStreak ?? 0) + 1,
      turnCount: (ledger.progress?.turnCount ?? 0) + 1,
    };
    ledger.lastFailingChecks = evaluation.failing;
    ledger.updatedAt = nowIso();
    saveRunLedger(activeRepoCwd, ledger);

    await writeStatusWidget(activeRepoCwd, ctx.ui, ledger, evaluation.failing);

    if (evaluation.allPass) {
      ledger.status = "done";
      ledger.updatedAt = nowIso();
      ledger.progress = {
        ...(ledger.progress ?? { failureStreak: 0, turnCount: 0 }),
        lastOutcome: "done",
      };
      saveRunLedger(activeRepoCwd, ledger);
      stopHeartbeat();
      armed = false;

      if (ctx.hasUI) ctx.ui.notify("Autopilot: done. Checks passed.", "info");

      // Terminal behavior: stop ticking.
      // Switch back to parent session is not implemented in v1.
      return;
    }

    // If checks failed, prompt continuation.
    const failing = evaluation.failing;
    const promptHash = shortHash(
      JSON.stringify({ failing, turnCount: ledger.progress?.turnCount ?? 0 }),
    );

    if (ledger.progress?.lastPromptHash && ledger.progress.lastPromptHash === promptHash) {
      // loop guard: we already asked for this exact failure set.
      const streak = ledger.progress?.failureStreak ?? 0;
      if (streak >= 2) {
        ledger.status = "awaiting-user";
        ledger.updatedAt = nowIso();
        saveRunLedger(activeRepoCwd, ledger);
        await writeStatusWidget(activeRepoCwd, ctx.ui, ledger, failing);
        ctx.ui.notify("Autopilot blocked: repeated failure set. Awaiting user.", "warning");
        return;
      }
    }

    ledger.progress = {
      ...(ledger.progress ?? { failureStreak: 0, turnCount: 0 }),
      lastPromptHash: promptHash,
      lastOutcome: "continue",
    };
    saveRunLedger(activeRepoCwd, ledger);

    const nextPrompt = buildNextPrompt({
      manifest,
      failingChecks: failing,
      iteration: ledger.progress.turnCount,
    });

    // Give user a chance to intervene on repeated failures, based on our tiered policy.
    failureStreak = ledger.progress.failureStreak;

    // Send next user message; agent is idle (agent_end), so this will trigger a new turn.
    pi.sendUserMessage(nextPrompt);
  }

  async function startAutopilotRun(opts: {
    ctx: ExtensionContext;
    repoCwd: string;
    manifestPath: string;
    force?: boolean;
  }) {
    const { ctx, repoCwd, manifestPath, force = false } = opts;
    const m = parseManifest(manifestPath);
    const slug = slugify(m.frontmatter.id ?? m.file) || shortHash(m.file);
    const runId = `run_${shortHash(m.file + nowIso())}`;

    const lease = await acquireOrTakeLease({
      ctx,
      repoCwd,
      slug,
      runId,
      manifestPath: m.file,
      manifestId: m.frontmatter.id,
      force,
    });

    if (!lease.ok || !lease.leaseOwner) {
      ctx.ui.notify(`Autopilot start blocked: ${lease.reason}`, "warning");
      return false;
    }

    const ledger: RunLedger = {
      runId,
      manifestPath: m.file,
      manifestId: m.frontmatter.id,
      status: "running",
      branch: m.frontmatter.branch,
      createdAt: nowIso(),
      updatedAt: nowIso(),
      leaseOwner: lease.leaseOwner,
      progress: {
        failureStreak: 0,
        turnCount: 0,
        lastOutcome: "started",
      },
    };

    const { runsDir } = getAutopilotDirs(repoCwd);
    ensureDir(runsDir);
    saveRunLedger(repoCwd, ledger);

    // load/check state and arm
    updateInMemoryAfterLoad(repoCwd, slug, runId, m, ledger);

    startHeartbeat(repoCwd, slug, ledger.leaseOwner!);

    ctx.ui.notify(`Autopilot armed: ${m.frontmatter.id ?? path.basename(m.file)} (${runId.slice(0, 8)})`, "info");

    // immediate tick: run verification before continuing
    await autopilotTick(ctx);
    return true;
  }

  pi.registerTool({
    name: "autopilot_transition",
    label: "Autopilot Transition",
    description: "Request a validated Autopilot v2 phase transition or human approval gate.",
    promptSnippet: "Request an Autopilot v2 phase transition or approval gate from the extension validator.",
    promptGuidelines: [
      "Use autopilot_transition when Autopilot planning drafts are ready and the next phase needs validation or approval.",
      "Use targetPhase/phase for validator-only transitions such as issue-approval, execution-approval, done, or blocked.",
      "Use gate for human approval gates: issues, before-issues, execution, or before-execution.",
      "Do not use autopilot_transition until the required local artifacts for the requested phase/gate are drafted.",
    ],
    parameters: AutopilotTransitionParams as any,
    async execute(
      _toolCallId: string,
      params: any,
      _signal: AbortSignal | undefined,
      _onUpdate: ((result: any) => void) | undefined,
      ctx: ExtensionContext,
    ) {
      const repoCwd = await resolveV2RepoCwd(pi, ctx.cwd);
      const gateToken = params.gate ?? "";
      const gate = gateToken ? parseApprovalGateToken(gateToken) : null;
      const phaseToken = params.targetPhase ?? params.phase ?? (gate ? "" : gateToken);

      if (phaseToken) {
        if (!v2.isWorkflowPhase(phaseToken)) {
          return {
            content: [{ type: "text", text: `Autopilot transition rejected: unknown phase ${phaseToken}.` }],
            details: { transitioned: false, phase: phaseToken, reasons: [`Unknown phase: ${phaseToken}`] },
          };
        }

        const workflow = resolveWorkflowOrNotify(ctx, repoCwd, params.workflowId ?? "");
        if (!workflow) {
          return {
            content: [{ type: "text", text: "Autopilot transition rejected: workflow not found." }],
            details: { transitioned: false, phase: phaseToken, reasons: ["Workflow not found"] },
          };
        }

        const result = v2.transitionWorkflowPhase(workflow, phaseToken, {
          actor: "autopilot_transition",
          note: params.note,
          evidencePaths: params.evidencePaths,
          force: Boolean(params.force),
        });

        if (!result.ok) {
          const reasons = result.validation.reasons;
          return {
            content: [{ type: "text", text: `Autopilot transition rejected: ${reasons.join(" ")}` }],
            details: { transitioned: false, workflowId: workflow.workflowId, from: workflow.phase, to: phaseToken, reasons },
          };
        }

        return {
          content: [{ type: "text", text: `Autopilot transition accepted: ${workflow.phase} -> ${result.state.phase}.` }],
          details: { transitioned: true, workflowId: workflow.workflowId, from: workflow.phase, phase: result.state.phase },
        };
      }

      if (!gate) {
        throw new Error("Provide either targetPhase/phase or gate. gate must be one of: issues, before-issues, execution, before-execution");
      }

      const workflow = await approveWorkflowGateWithUi({
        pi,
        ctx,
        repoCwd,
        gate,
        selector: params.workflowId ?? "",
        note: params.note,
      });

      if (!workflow) {
        return {
          content: [{ type: "text", text: `Autopilot ${gate} approval was not granted.` }],
          details: { approved: false, gate },
        };
      }

      return {
        content: [{ type: "text", text: `Autopilot ${gate} approved for ${workflow.workflowId}. Continue with the next gated phase.` }],
        details: { approved: true, gate, workflowId: workflow.workflowId, phase: workflow.phase },
      };
    },
  } as any);

  // Commands
  pi.registerCommand("autopilot", {
    description: "Autopilot v2 workflow engine (plan/approve/transition/ship)",
    handler: async (args, ctx) => {
      const repoCwd = await resolveV2RepoCwd(pi, ctx.cwd);
      const normalizedArgs = normalizeAutopilotCommandArgs(args);
      const cmd = normalizedArgs.split(/\s+/)[0] ?? "";
      const rest = normalizedArgs.slice(cmd.length).trim();

      if (!cmd) {
        ctx.ui.notify(
          "Usage: /autopilot <plan|approve|transition|ship> [idea|issues workflowId|execution workflowId|workflowId]",
          "info",
        );
        return;
      }

      const recognizedCommands = new Set([
        "plan",
        "approve",
        "ship",
        "transition",
        "concept-lock",
        "lock-concept",
        "status",
        "workflows",
        "workflow",
        "setup",
        "prefs",
        "architecture",
        "arch",
        "resume-workflow",
        "legacy-plan",
        "plan-legacy",
        "continue",
        "from-gh",
        "from-linear",
        "scaffold",
        "init",
      ]);
      if (!recognizedCommands.has(cmd)) {
        ctx.ui.notify(
          `Unknown Autopilot subcommand: ${cmd}. Use /autopilot plan, /autopilot approve issues, /autopilot approve execution, /autopilot transition, or /autopilot ship.`,
          "warning",
        );
        return;
      }

      if (cmd === "status") {
        const allowed = checkRepoAllowlisted(pi, repoCwd);
        ctx.ui.notify(buildStatusSummary(repoCwd, allowed), "info");
        return;
      }

      if (cmd === "workflows" || cmd === "workflow") {
        const workflows = v2.listWorkflowStates(repoCwd);
        if (!workflows.length) {
          ctx.ui.notify("No v2 workflows found. Use /autopilot plan <idea> first.", "info");
          return;
        }
        ctx.ui.notify(workflows.slice(0, 8).map((workflow) => v2.summarizeWorkflow(workflow)).join("\n\n"), "info");
        return;
      }

      if (cmd === "setup" || cmd === "prefs") {
        const parsed = parseSetupCommandArgs(rest);
        if (parsed.help) {
          ctx.ui.notify(
            "Usage: /autopilot setup [--source plan|github|linear|mixed] [--verify conservative|normal|strict] [--allow packages/**,app/**,docs/**,opensrc/**] [--deny node_modules/**] [--interactive]",
            "info",
          );
          return;
        }

        try {
          const result = parsed.interactive
            ? await setupAutopilotRepo(ctx, getGlobalConfigPath())
            : await setupAutopilotRepoWithOptions(repoCwd, getGlobalConfigPath(), parsed.options);
          v2.ensureRepoV2Scaffold(repoCwd);
          ctx.ui.notify(
            `Autopilot configured. Repo enabled at ${result.configPath}. Global allowlist updated at ${result.allowlistPath}. ` +
              `Profile=${result.preferences.verificationProfile}, sources=${result.preferences.sourcePriority.join(">")}.`,
            "info",
          );
        } catch (error) {
          ctx.ui.notify(
            `Autopilot setup failed: ${error instanceof Error ? error.message : String(error)}`,
            "error",
          );
        }
        return;
      }

      let repoAllowed = checkRepoAllowlisted(pi, repoCwd);
      if (!repoAllowed) {
        repoAllowed = await ensureAutopilotReadyForRepo(ctx, repoCwd);
        if (!repoAllowed) return;
      }

      if (cmd === "plan") {
        let topic = rest;
        if (!topic) {
          const value = await ctx.ui.input("Autopilot v2 plan", "What idea, source, URL, issue, repo, package, or plan should we work from?");
          if (!value?.trim()) {
            ctx.ui.notify("Autopilot plan cancelled.", "warning");
            return;
          }
          topic = value.trim();
        }

        createAndLaunchV2Workflow({ pi, ctx, repoCwd, lane: "planning", input: topic });
        return;
      }

      if (cmd === "legacy-plan" || cmd === "plan-legacy") {
        let topic = rest;
        if (!topic) {
          const value = await ctx.ui.input("Autopilot legacy plan", "What do you want to plan?");
          if (!value?.trim()) {
            ctx.ui.notify("Autopilot legacy plan cancelled.", "warning");
            return;
          }
          topic = value.trim();
        }

        const plansDir = path.join(repoCwd, ".pi", "autopilot", "plans");
        ensureDir(plansDir);
        const planSlug = slugify(topic).slice(0, 80) || `plan-${Date.now()}`;
        const outputPath = path.join(plansDir, `${planSlug}.md`);
        const prompt = buildAutopilotPlanPrompt({ repoCwd, topic, outputPath });
        sendAutopilotPrompt(pi, ctx, prompt);
        ctx.ui.notify(`Autopilot legacy planning started. Target spec: ${outputPath}`, "info");
        return;
      }

      if (cmd === "architecture" || cmd === "arch") {
        let scope = rest;
        if (!scope) {
          const value = await ctx.ui.input("Autopilot architecture", "What codebase area or friction should the architecture lane inspect?");
          if (!value?.trim()) {
            ctx.ui.notify("Autopilot architecture cancelled.", "warning");
            return;
          }
          scope = value.trim();
        }

        createAndLaunchV2Workflow({ pi, ctx, repoCwd, lane: "architecture", input: scope });
        return;
      }

      if (cmd === "approve") {
        await approveV2WorkflowGate({ pi, ctx, repoCwd, rest });
        return;
      }

      if (cmd === "transition") {
        await transitionV2WorkflowPhase({ ctx, repoCwd, rest });
        return;
      }

      if (cmd === "concept-lock" || cmd === "lock-concept") {
        const parts = rest.split(/\s+/).filter(Boolean);
        const selector = parts[0] ?? "";
        let workflow = selector ? resolveWorkflowOrNotify(ctx, repoCwd, selector) : resolveWorkflowOrNotify(ctx, repoCwd, "");
        let summary = selector ? parts.slice(1).join(" ") : "";
        if (!workflow && selector) {
          workflow = resolveWorkflowOrNotify(ctx, repoCwd, rest);
          summary = "";
        }
        if (!workflow) return;
        try {
          const locked = v2.recordConceptLock(workflow, {
            summary: summary || "Concept lock accepted; continue to PRD/glossary/issue drafting.",
            acceptedBy: "slash-command",
          });
          sendAutopilotPrompt(pi, ctx, v2.buildArtifactDraftingPrompt(locked));
          ctx.ui.notify(`Concept lock recorded for ${locked.workflowId}. Artifact drafting prompt sent.`, "info");
        } catch (error) {
          ctx.ui.notify(`Concept lock failed: ${error instanceof Error ? error.message : String(error)}`, "warning");
        }
        return;
      }

      if (cmd === "ship") {
        const workflow = resolveWorkflowOrNotify(ctx, repoCwd, rest);
        if (!workflow) return;
        if (workflow.gates["before-execution"].status !== "approved") {
          ctx.ui.notify(`Execution is blocked. Run /autopilot approve execution ${workflow.workflowId} first.`, "warning");
          return;
        }
        try {
          const claim = v2.claimNextExecutionIssue(workflow, { actor: "slash-command", mode: "git-worktree" });
          const prepared = prepareFreshIssueWorktree({
            repoCwd,
            worktreePath: claim.executionState.worktreePath!,
            branch: claim.executionState.branch!,
          });
          const prefs = loadRepoAutopilotPreferences(repoCwd);
          const runner = createNativeRunner({
            config: {
              commandTemplate: prefs.runnerCommandTemplate,
              concurrency: prefs.runnerConcurrency,
              maxRepairAttempts: prefs.runnerMaxRepairAttempts,
              idleTimeoutSeconds: prefs.runnerIdleTimeoutSeconds,
              envAllowlist: prefs.runnerEnvAllowlist,
              evidenceProfile: "concise",
            },
          });
          const logPath = path.join(workflow.paths.artifactsDir, "worker-logs", `${claim.issue.id}.implementation.log`);
          ctx.ui.notify(
            `Shipping issue ${claim.issue.id}: ${claim.issue.title}. Worktree: ${prepared.worktreePath}`,
            "info",
          );
          const result = await runner.runWorker({
            repoCwd,
            worktreePath: prepared.worktreePath,
            promptPath: claim.implementationPromptPath,
            workflowId: workflow.workflowId,
            issueId: String(claim.issue.id),
          }, logPath);
          if (result.exitCode !== 0) {
            const latest = v2.loadWorkflowState(repoCwd, workflow.workflowId) ?? workflow;
            v2.recordExecutionFailure(latest, String(claim.issue.id), {
              nonAmbiguous: true,
              message: `Worker failed with exit code ${result.exitCode}. Log: ${logPath}`,
            });
            ctx.ui.notify(`Autopilot worker failed for ${claim.issue.id}. Log: ${logPath}`, "warning");
            return;
          }
          ctx.ui.notify(`Autopilot worker completed for ${claim.issue.id}. Log: ${logPath}`, "info");
        } catch (error) {
          ctx.ui.notify(`Autopilot ship blocked: ${error instanceof Error ? error.message : String(error)}`, "warning");
        }
        return;
      }

      if (cmd === "resume-workflow") {
        const workflow = resolveWorkflowOrNotify(ctx, repoCwd, rest);
        if (!workflow) return;
        const prompt = workflow.lane === "architecture"
          ? v2.buildArchitecturePrompt(workflow)
          : v2.buildPlanningPrompt(workflow);
        sendAutopilotPrompt(pi, ctx, prompt);
        ctx.ui.notify(`Resuming v2 workflow: ${workflow.workflowId}`, "info");
        return;
      }

      if (cmd === "continue" || cmd === "from-gh" || cmd === "from-linear") {
        if (!rest) {
          ctx.ui.notify(
            "Usage: /autopilot continue <planPath|gh issue #123|#123|owner/repo#123|https://github.com/owner/repo/issues/123|ENG-123|https://linear.app/...>",
            "error",
          );
          return;
        }

        try {
          const { manifestPath } = await buildContinueManifest(pi, repoCwd, rest);
          if (await maybeStartAutopilotInIsolatedWorktree({ pi, ctx, repoCwd, manifestPath })) {
            return;
          }
          await startAutopilotRun({ ctx, repoCwd, manifestPath });
        } catch (error) {
          ctx.ui.notify(
            `Autopilot continue failed: ${error instanceof Error ? error.message : String(error)}`,
            "error",
          );
        }
        return;
      }

      if (cmd === "scaffold" || cmd === "init") {
        const slugOrPath = rest;
        if (!slugOrPath) {
          ctx.ui.notify("Usage: /autopilot scaffold <slug-or-manifestPath>", "error");
          return;
        }

        const defaultDir = path.join(repoCwd, ".pi", "autopilot");
        const manifestPath = slugOrPath.includes("/") || slugOrPath.startsWith(".")
          ? path.isAbsolute(slugOrPath)
            ? slugOrPath
            : path.join(repoCwd, slugOrPath)
          : path.join(defaultDir, `${slugify(slugOrPath)}.md`);

        ensureDir(path.dirname(manifestPath));

        let gitBranch = "";
        try {
          const { stdout } = await pi.exec("bash", ["-lc", "git rev-parse --abbrev-ref HEAD"], {
            timeout: 10_000,
          } as any);
          gitBranch = String(stdout ?? "").trim();
        } catch {
          // ignore
        }

        const id = slugify(path.basename(manifestPath).replace(/\.md$/i, "")) || "autopilot-run";

        const template = `---\n` +
`id: ${id}\n` +
`branch: ${gitBranch || ""}\n` +
`authority:\n  commit: false\n  push: true\n  pr: true\n  merge: false\n` +
`scope:\n  todo_ids: []\n` +
`paths:\n  allow:\n    - packages/**\n    - app/**\n    - docs/**\n    - opensrc/**\n  related: []\n  deny:\n    - node_modules/**\n` +
`checks:\n  - type: command\n    run: bun run typecheck\n  - type: command\n    run: cd packages/core && bun test src/server-contract.test.ts\n  - type: command\n    run: cd app && bun run test src/features/strategy/__tests__/hooks.test.ts src/features/strategy/__tests__/StrategyComposerView.test.tsx\nverify:\n  - all_required_checks_passed\nacceptance:\n  - required checks pass\nstop_when:\n  - all_required_checks_passed\n` +
`---\n\n## Context\nWrite short context + what you want done.\n\n## Execution notes\nOptional notes for the supervisor (avoid authority changes).\n`;

        fs.writeFileSync(manifestPath, template, "utf-8");
        ctx.ui.notify(`Autopilot manifest scaffolded: ${manifestPath}`, "info");
        return;
      }

      if (cmd === "from-plan" || cmd === "draft") {
        const planPath = rest;
        if (!planPath) {
          ctx.ui.notify("Usage: /autopilot from-plan <planPath>", "error");
          return;
        }

        const absPlan = path.isAbsolute(planPath)
          ? planPath
          : path.join(repoCwd, planPath);
        if (!fileExists(absPlan)) {
          ctx.ui.notify(`Plan file not found: ${absPlan}`, "error");
          return;
        }

        const rawPlan = readText(absPlan);
        const planSlug = slugify(path.basename(absPlan).replace(/\.[^.]+$/, ""));
        const manifestDir = path.join(repoCwd, ".pi", "autopilot");
        ensureDir(manifestDir);
        const manifestPath = path.join(manifestDir, `${planSlug || "autopilot"}.md`);

        // Cheap heuristic: add targeted checks if plan mentions key areas.
        const checks: string[] = [
          "  - type: command\n    run: bun run typecheck",
          "  - type: command\n    run: cd packages/core && bun test src/server-contract.test.ts",
        ];

        const planLower = rawPlan.toLowerCase();
        if (planLower.includes("strategy composer") || planLower.includes("strategy") && planLower.includes("dry-run")) {
          checks.push(
            "  - type: command\n    run: cd app && bun run test src/features/strategy/__tests__/hooks.test.ts src/features/strategy/__tests__/StrategyComposerView.test.tsx",
          );
        }

        let gitBranch = "";
        try {
          const { stdout } = await pi.exec("bash", ["-lc", "git rev-parse --abbrev-ref HEAD"], { timeout: 10_000 } as any);
          gitBranch = String(stdout ?? "").trim();
        } catch {
          // ignore
        }

        const template =
          "---\n" +
          `id: ${planSlug || "autopilot"}\n` +
          `branch: ${gitBranch || ""}\n` +
          "authority:\n  commit: false\n  push: true\n  pr: true\n  merge: false\n" +
          "scope:\n  todo_ids: []\n" +
          "paths:\n  allow:\n    - packages/**\n    - app/**\n    - docs/**\n    - opensrc/**\n  related: []\n  deny:\n    - node_modules/**\n" +
          "checks:\n" +
          checks.join("\n") +
          "\nverify:\n  - all_required_checks_passed\n---\n\n" +
          "## Source plan\n" +
          "" +
          rawPlan +
          "\n";

        fs.writeFileSync(manifestPath, template, "utf-8");
        ctx.ui.notify(`Autopilot manifest drafted from plan: ${manifestPath}`, "info");
        return;
      }

      if (cmd === "start") {
        const sourceOrManifest = rest;
        if (!sourceOrManifest) {
          ctx.ui.notify(
            "Usage: /autopilot start <manifestPath|planPath|gh issue #123|#123|owner/repo#123|https://github.com/owner/repo/issues/123>",
            "error",
          );
          return;
        }

        try {
          const resolved = await resolveStartManifestPath(pi, repoCwd, sourceOrManifest);
          if (resolved.mode === "generated") {
            ctx.ui.notify(`Autopilot manifest generated: ${resolved.manifestPath}`, "info");
          }
          if (await maybeStartAutopilotInIsolatedWorktree({ pi, ctx, repoCwd, manifestPath: resolved.manifestPath })) {
            return;
          }
          await startAutopilotRun({ ctx, repoCwd, manifestPath: resolved.manifestPath });
        } catch (error) {
          ctx.ui.notify(
            `Autopilot start failed: ${error instanceof Error ? error.message : String(error)}`,
            "error",
          );
        }
        return;
      }

      if (cmd === "pause") {
        if (!activeRepoCwd || !activeRunId) {
          ctx.ui.notify("No active autopilot run.", "warning");
          return;
        }
        await markStatus(repoCwd, activeRunId, "paused", ctx);
        armed = false;
        stopHeartbeat();
        ctx.ui.notify("Autopilot paused.", "info");
        return;
      }

      if (cmd === "resume") {
        if (!activeRepoCwd || !activeRunId || !activeSlug || !manifest) {
          ctx.ui.notify("No in-memory run to resume. Use /autopilot takeover or start.", "warning");
          return;
        }
        await markStatus(repoCwd, activeRunId, "running", ctx, lastFailingChecks);
        armed = true;
        // re-acquire heartbeat by reloading lease owner
        const owner = loadLeaseOwner(repoCwd, activeSlug);
        if (owner && owner.runId === activeRunId) {
          startHeartbeat(repoCwd, activeSlug, owner);
        }
        ctx.ui.notify("Autopilot resumed.", "info");
        await autopilotTick(ctx);
        return;
      }

      if (cmd === "stop") {
        await stopRun(repoCwd);
        ctx.ui.notify("Autopilot stopped.", "info");
        return;
      }

      if (cmd === "checkpoint") {
        if (!activeRepoCwd || !activeRunId || !manifest) {
          ctx.ui.notify("No active run.", "warning");
          return;
        }
        const ledger = loadRunLedger(repoCwd, activeRunId);
        if (!ledger) return;
        ledger.updatedAt = nowIso();
        saveRunLedger(repoCwd, ledger);
        ctx.ui.notify("Checkpoint written.", "info");
        return;
      }

      if (cmd === "takeover") {
        const manifestPath = rest;
        if (!manifestPath) {
          ctx.ui.notify("Usage: /autopilot takeover <manifestPath>", "error");
          return;
        }

        try {
          const started = await startAutopilotRun({ ctx, repoCwd, manifestPath, force: true });
          if (started) {
            ctx.ui.notify("Takeover complete; continuing.", "info");
          }
        } catch (error) {
          ctx.ui.notify(
            `Autopilot takeover failed: ${error instanceof Error ? error.message : String(error)}`,
            "error",
          );
        }
        return;
      }

      if (cmd === "handoff") {
        ctx.ui.notify("handoff not implemented in v1 skeleton.", "warning");
        return;
      }

      ctx.ui.notify(`Unknown autopilot command: ${cmd}`, "warning");
    },
  });

  pi.on("tool_call", async (event, ctx) => {
    const repoCwd = await resolveV2RepoCwd(pi, ctx.cwd);

    if ((event.toolName === "write" || event.toolName === "edit") && typeof event.input?.path === "string") {
      const reason = lockedPlanningArtifactWriteReason(repoCwd, event.input.path);
      if (reason) return { block: true, reason };
    }

    if (event.toolName === "bash" && typeof event.input?.command === "string") {
      const artifactReason = lockedPlanningArtifactCommandReason(repoCwd, event.input.command);
      if (artifactReason) return { block: true, reason: artifactReason };
    }

    if (!armed) return;

    if (event.toolName === "bash" && typeof event.input?.command === "string") {
      const reason = shouldBlockSensitiveCommand(event.input.command);
      if (reason) {
        return { block: true, reason };
      }
      return;
    }

    if (event.toolName === "read" && typeof event.input?.path === "string" && isSensitiveReadPath(event.input.path)) {
      return { block: true, reason: `Refusing to read secret-bearing path during autopilot: ${event.input.path}` };
    }
  });

  pi.on("tool_result", async (event) => {
    if (!armed) return;
    if (event.toolName !== "bash" && event.toolName !== "read") return;

    let changed = false;
    const content = event.content.map((item) => {
      if (item.type !== "text") return item;
      const redacted = redactSensitiveText(item.text);
      if (redacted !== item.text) {
        changed = true;
        return { ...item, text: redacted };
      }
      return item;
    });

    if (changed) {
      return { content };
    }
  });

  // Auto-tick after each agent completion.
  pi.on("agent_end", async (_event, ctx) => {
    try {
      await autopilotTick(ctx);
    } catch (e) {
      if (ctx.hasUI) {
        ctx.ui.notify(
          `Autopilot tick error: ${e instanceof Error ? e.message : String(e)}`,
          "warning",
        );
      }
    }
  });

  // Heartbeat / status update on session start if a run is already armed.
  pi.on("session_start", async (_event, ctx) => {
    try {
      // Load repo allowlist gating, but don't auto-resume automatically in v1.
      // Keep this minimal to avoid unexpected takeover.
      if (!checkRepoAllowlisted(pi, ctx.cwd)) return;

      const repoCwd = ctx.cwd;
      const root = getRepoAutopilotRoot(repoCwd);
      const { runsDir, locksDir } = getAutopilotDirs(repoCwd);

      const leaseFiles = fileExists(locksDir)
        ? fs.readdirSync(locksDir).filter((f) => f.endsWith(".json"))
        : [];

      if (!leaseFiles.length) return;

      // Show a quick notification if stale locks exist.
      const staleAfterMs = 120_000;
      for (const lf of leaseFiles) {
        const owner = loadLeaseOwner(repoCwd, lf.replace(/\.json$/, ""));
        if (!owner) continue;
        if (isLeaseStale(owner, staleAfterMs)) {
          const ledger = loadRunLedger(repoCwd, owner.runId);
          if (ledger && ledger.status !== "done" && ledger.status !== "stopped") {
            ctx.ui.notify(
              `Autopilot: stale run detected (${ledger.manifestId ?? ledger.manifestPath}). Use /autopilot takeover <manifestPath>.`,
              "warning",
            );
          }
        }
      }

      // Also update if we have an in-memory run (hot reload scenario).
      if (armed && activeRepoCwd === repoCwd && activeRunId) {
        const ledger = loadRunLedger(repoCwd, activeRunId);
        if (ledger) {
          await writeStatusWidget(repoCwd, ctx.ui, ledger, lastFailingChecks);
        }
      }
    } catch {
      // ignore
    }
  });
}
