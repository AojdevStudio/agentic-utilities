import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, resolve } from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { DynamicBorder, isToolCallEventType } from "@mariozechner/pi-coding-agent";
import type { SelectItem } from "@mariozechner/pi-tui";
import { Container, SelectList, Text } from "@mariozechner/pi-tui";
import { parse as shellParse } from "shell-quote";

type Severity = "high" | "medium";
type GuardAction = "allow" | "prompt" | "block";
type GuardTool = "bash" | "read" | "write" | "edit" | "*";
type PromptChoice = "run" | "always_allow" | "abort";

type Risk = {
  severity: Severity;
  reasons: string[];
};

type OpToken = { op: string; [k: string]: unknown };
type Token = string | OpToken;

type PolicyRule = {
  tool?: GuardTool;
  pattern: string;
  reason?: string;
  severity?: Severity;
};

type PolicyFile = {
  bash?: Partial<Record<GuardAction, PolicyRule[]>>;
  paths?: Partial<Record<GuardAction, PolicyRule[]>>;
};

const EXTENSION_NAME = "bash-guard";
const ABORT_REMEMBER_MS = 60_000;

function isOpToken(t: Token): t is OpToken {
  return typeof t === "object" && t !== null && "op" in t;
}

function tokensToStrings(tokens: Token[]): string[] {
  return tokens.filter((t): t is string => typeof t === "string");
}

function splitOnOps(tokens: Token[], splitOps: string[]): Token[][] {
  const out: Token[][] = [];
  let current: Token[] = [];
  for (const t of tokens) {
    if (isOpToken(t) && splitOps.includes(t.op)) {
      if (current.length) out.push(current);
      current = [];
      continue;
    }
    current.push(t);
  }
  if (current.length) out.push(current);
  return out;
}

function hasShortFlag(args: string[], flag: string): boolean {
  if (!flag.startsWith("-") || flag.length !== 2) return args.includes(flag);
  const letter = flag[1];
  return args.some((arg) => arg === flag || (arg.startsWith("-") && !arg.startsWith("--") && arg.includes(letter)));
}

function anyArgStartsWith(args: string[], prefix: string): boolean {
  return args.some((a) => a.startsWith(prefix));
}

function mergeRisk(base: Risk | null, next: Risk | null): Risk | null {
  if (!base) return next;
  if (!next) return base;
  return {
    severity: base.severity === "high" || next.severity === "high" ? "high" : "medium",
    reasons: [...new Set([...base.reasons, ...next.reasons])],
  };
}

function hasRmRecursiveForce(args: string[]): boolean {
  let hasRecursive = false;
  let hasForce = false;
  for (const arg of args) {
    if (arg === "--recursive") hasRecursive = true;
    if (arg === "--force") hasForce = true;
    if (arg.startsWith("-") && !arg.startsWith("--")) {
      if (arg.includes("r") || arg.includes("R")) hasRecursive = true;
      if (arg.includes("f")) hasForce = true;
    }
  }
  return hasRecursive && hasForce;
}

function isProtectedRmTarget(target: string): boolean {
  return (
    target === "/" ||
    target.startsWith("/:") ||
    target === "~" ||
    target.startsWith("~:") ||
    target === "~/" ||
    target === "~/.claude" ||
    target.startsWith("~/.claude/") ||
    target.startsWith("~/.claude:")
  );
}

function analyzeArgv(argv: string[]): Risk | null {
  if (argv.length === 0) return null;
  const [cmd, ...rest] = argv;

  if (cmd === "sudo") {
    const nested = analyzeArgv(rest);
    if (nested) return { severity: "high", reasons: nested.reasons.map((reason) => `sudo ${reason}`) };
    return null;
  }

  if (cmd === "rm" && hasRmRecursiveForce(rest)) {
    const targets = rest.filter((arg) => !arg.startsWith("-"));
    const protectedTarget = targets.find(isProtectedRmTarget);
    if (protectedTarget) {
      return { severity: "high", reasons: [`rm -rf ${protectedTarget} (protected destructive delete)`] };
    }
  }

  if (cmd === "diskutil") {
    const joined = rest.join(" ");
    if (/^(eraseDisk|zeroDisk|partitionDisk)\b/.test(joined)) {
      return { severity: "high", reasons: [`diskutil ${rest[0]} (destructive disk operation)`] };
    }
    if (/^apfs\s+(deleteContainer|eraseVolume)\b/.test(joined)) {
      return {
        severity: "high",
        reasons: [`diskutil ${joined.split(/\s+/).slice(0, 2).join(" ")} (destructive APFS operation)`],
      };
    }
  }

  if (cmd === "dd" && rest.some((arg) => arg.startsWith("if=/dev/zero"))) {
    return { severity: "high", reasons: ["dd if=/dev/zero (can overwrite data when paired with an output)"] };
  }

  if (cmd.startsWith("mkfs")) {
    return { severity: "high", reasons: [`${cmd} (filesystem formatting)`] };
  }

  if (cmd === "gh" && rest[0] === "repo" && rest[1] === "delete") {
    return { severity: "high", reasons: ["gh repo delete (repository deletion)"] };
  }

  if (cmd === "gh" && rest[0] === "repo" && rest[1] === "edit") {
    const visibilityIndex = rest.indexOf("--visibility");
    if (visibilityIndex >= 0 && rest[visibilityIndex + 1] === "public") {
      return { severity: "high", reasons: ["gh repo edit --visibility public (repository exposure)"] };
    }
  }

  if (cmd === "git" && rest[0] === "push" && rest.some((arg) => arg === "--force" || arg === "-f")) {
    return { severity: "high", reasons: ["git push --force/-f (rewrite remote history)"] };
  }

  return null;
}

export function analyzeBashSegment(seg: Token[]): Risk | null {
  return analyzeArgv(tokensToStrings(seg));
}

export function analyzeBashCommand(command: string): Risk | null {
  let tokens: Token[];
  try {
    tokens = shellParse(command) as Token[];
  } catch {
    return null;
  }

  let risk: Risk | null = null;
  for (const seg of splitOnOps(tokens, ["&&", "||", ";", "|"])) {
    risk = mergeRisk(risk, analyzeBashSegment(seg));
  }
  return risk;
}

const HEADLESS_BLOCKED: Array<{ pattern: RegExp; reason: string }> = [
  {
    pattern: /\bsudo\s+rm\b[^#\n]*-[a-zA-Z]*r[a-zA-Z]*f?[^#\n]*\s(\/|~|~\/\.claude)(?:\s|$|\/|:)/,
    reason: "sudo rm -rf against protected root/home path",
  },
  {
    pattern: /\brm\b[^#\n]*-[a-zA-Z]*r[a-zA-Z]*f?[^#\n]*\s(\/|~|~\/\.claude)(?:\s|$|\/|:)/,
    reason: "rm -rf against protected root/home path",
  },
  { pattern: /\bdiskutil\s+(eraseDisk|zeroDisk|partitionDisk)\b/i, reason: "destructive diskutil operation" },
  { pattern: /\bdiskutil\s+apfs\s+(deleteContainer|eraseVolume)\b/i, reason: "destructive diskutil APFS operation" },
  { pattern: /\bdd\b[^#\n]*\bif=\/dev\/zero\b/, reason: "zero-fill write source (dd if=/dev/zero)" },
  { pattern: /\bmkfs/, reason: "filesystem formatting (mkfs)" },
  { pattern: /\bgh\s+repo\s+delete\b/, reason: "repository deletion (gh repo delete)" },
  {
    pattern: /\bgh\s+repo\s+edit\b[^#\n]*--visibility\s+public\b/,
    reason: "repository exposure (gh repo edit --visibility public)",
  },
  { pattern: /\bgit\s+push\b[^#\n]*(--force|-f)\b/, reason: "rewrite remote history (git push --force/-f)" },
];

const DEFAULT_PATH_BLOCKS: PolicyRule[] = [];

const DEFAULT_PATH_PROMPTS: PolicyRule[] = [
  { tool: "read", pattern: "~/.ssh/id_*", reason: "private SSH key read" },
  { tool: "read", pattern: "~/.ssh/*.pem", reason: "private SSH material read" },
  { tool: "read", pattern: "~/.aws/credentials", reason: "AWS credential file read" },
  { tool: "read", pattern: "~/.gnupg/private*", reason: "GnuPG private key material read" },
  { tool: "write", pattern: "~/.claude/settings.json", reason: "Claude Code settings write" },
  { tool: "edit", pattern: "~/.claude/settings.json", reason: "Claude Code settings edit" },
  { tool: "write", pattern: "~/.ssh/*", reason: "SSH configuration/key write" },
  { tool: "edit", pattern: "~/.ssh/*", reason: "SSH configuration/key edit" },
];

function expandHome(input: string): string {
  if (input === "~") return homedir();
  if (input.startsWith("~/")) return `${homedir()}${input.slice(1)}`;
  return input;
}

function normalizePathForMatch(input: string, cwd: string): string[] {
  const expanded = expandHome(input);
  const absolute = isAbsolute(expanded) ? expanded : resolve(cwd, expanded);
  return [...new Set([input, expanded, absolute])];
}

function wildcardToRegExp(pattern: string): RegExp {
  const expanded = expandHome(pattern);
  const doubleStarSentinel = "__BASH_GUARD_DOUBLE_STAR__";
  const escaped = expanded
    .replace(/[|\\{}()[\]^$+?.]/g, "\\$&")
    .replace(/\*\*/g, doubleStarSentinel)
    .replace(/\*/g, "[^/]*")
    .replaceAll(doubleStarSentinel, ".*");
  return new RegExp(`^${escaped}$`);
}

function ruleMatchesText(rule: PolicyRule, value: string): boolean {
  try {
    return new RegExp(rule.pattern).test(value);
  } catch {
    return value.includes(rule.pattern);
  }
}

function ruleMatchesPath(rule: PolicyRule, tool: GuardTool, pathValue: string, cwd: string): boolean {
  if (rule.tool && rule.tool !== "*" && rule.tool !== tool) return false;
  const candidates = normalizePathForMatch(pathValue, cwd);
  const matchers = [rule.pattern, expandHome(rule.pattern)];
  return matchers.some((pattern) => {
    const glob = wildcardToRegExp(pattern);
    return candidates.some((candidate) => glob.test(candidate));
  });
}

function readPolicyFile(path: string): PolicyFile | null {
  if (!existsSync(path)) return null;
  try {
    return JSON.parse(readFileSync(path, "utf8")) as PolicyFile;
  } catch {
    return null;
  }
}

function loadPolicy(cwd: string): PolicyFile {
  const globalPolicy = readPolicyFile(`${homedir()}/.pi/agent/bash-guard-policy.json`) ?? {};
  const localPolicy = readPolicyFile(resolve(cwd, ".pi/bash-guard-policy.json")) ?? {};
  return {
    bash: {
      allow: [...(globalPolicy.bash?.allow ?? []), ...(localPolicy.bash?.allow ?? [])],
      prompt: [...(globalPolicy.bash?.prompt ?? []), ...(localPolicy.bash?.prompt ?? [])],
      block: [...(globalPolicy.bash?.block ?? []), ...(localPolicy.bash?.block ?? [])],
    },
    paths: {
      allow: [...(globalPolicy.paths?.allow ?? []), ...(localPolicy.paths?.allow ?? [])],
      prompt: [...DEFAULT_PATH_PROMPTS, ...(globalPolicy.paths?.prompt ?? []), ...(localPolicy.paths?.prompt ?? [])],
      block: [...DEFAULT_PATH_BLOCKS, ...(globalPolicy.paths?.block ?? []), ...(localPolicy.paths?.block ?? [])],
    },
  };
}

function applyBashPolicy(
  command: string,
  policy: PolicyFile,
): { action: GuardAction; risk?: Risk; reason?: string } | null {
  for (const rule of policy.bash?.allow ?? []) if (ruleMatchesText(rule, command)) return { action: "allow" };
  for (const rule of policy.bash?.block ?? []) {
    if (ruleMatchesText(rule, command))
      return { action: "block", reason: rule.reason ?? `policy block: ${rule.pattern}` };
  }
  for (const rule of policy.bash?.prompt ?? []) {
    if (ruleMatchesText(rule, command)) {
      return {
        action: "prompt",
        risk: { severity: rule.severity ?? "medium", reasons: [rule.reason ?? `policy prompt: ${rule.pattern}`] },
      };
    }
  }
  return null;
}

function applyPathPolicy(
  tool: GuardTool,
  pathValue: string,
  cwd: string,
  policy: PolicyFile,
): { action: GuardAction; risk?: Risk; reason?: string } | null {
  for (const rule of policy.paths?.allow ?? [])
    if (ruleMatchesPath(rule, tool, pathValue, cwd)) return { action: "allow" };
  for (const rule of policy.paths?.block ?? []) {
    if (ruleMatchesPath(rule, tool, pathValue, cwd))
      return { action: "block", reason: rule.reason ?? `path policy block: ${rule.pattern}` };
  }
  for (const rule of policy.paths?.prompt ?? []) {
    if (ruleMatchesPath(rule, tool, pathValue, cwd)) {
      return {
        action: "prompt",
        risk: { severity: rule.severity ?? "medium", reasons: [rule.reason ?? `path policy prompt: ${rule.pattern}`] },
      };
    }
  }
  return null;
}

async function promptRunOrAbort(
  ctx: ExtensionContext,
  title: string,
  subject: string,
  risk: Risk,
): Promise<PromptChoice> {
  if (!ctx.hasUI) return "abort";

  const body = `Flagged as ${risk.severity.toUpperCase()} risk:\n\n${risk.reasons.map((r) => `• ${r}`).join("\n")}\n\n${subject}`;
  const items: SelectItem[] = [
    { value: "run", label: "Run once", description: "Allow this tool call once" },
    { value: "always_allow", label: "Always allow", description: "Allow this exact command/path for this Pi session" },
    { value: "abort", label: "Abort", description: "Block this tool call" },
  ];

  const choice = await ctx.ui.custom<PromptChoice>(
    (tui, theme, _kb, done) => {
      const container = new Container();
      container.addChild(new DynamicBorder((s: string) => theme.fg("warning", s)));
      container.addChild(new Text(theme.fg("warning", theme.bold(title)), 1, 0));
      container.addChild(new Text(body, 1, 0));

      const list = new SelectList(items, items.length, {
        selectedPrefix: (t) => theme.fg("accent", t),
        selectedText: (t) => theme.fg("accent", t),
        description: (t) => theme.fg("muted", t),
        scrollInfo: (t) => theme.fg("dim", t),
        noMatch: (t) => theme.fg("warning", t),
      });

      list.onSelect = (item) => done(item.value as PromptChoice);
      list.onCancel = () => done("abort");
      container.addChild(list);
      container.addChild(new DynamicBorder((s: string) => theme.fg("warning", s)));

      return {
        render: (w) => container.render(w),
        invalidate: () => container.invalidate(),
        handleInput: (data) => {
          list.handleInput(data);
          tui.requestRender();
        },
      };
    },
    { overlay: true },
  );

  return choice ?? "abort";
}

function isSubagent(): boolean {
  const depth = Number(process.env.PI_SUBAGENT_DEPTH ?? "0");
  return Number.isFinite(depth) && depth >= 1;
}

export default function (pi: ExtensionAPI) {
  pi.registerFlag("bash-guard-auto-allow", {
    description:
      "Allow prompt-level bash-guard findings when no UI is available. Catastrophic headless blocks still apply.",
    type: "boolean",
    default: false,
  });

  pi.registerFlag("bash-guard-headless-allow-catastrophic", {
    description: "Dangerous escape hatch: allow catastrophic commands in subagent/headless mode.",
    type: "boolean",
    default: false,
  });

  pi.registerCommand("bash-guard", {
    description: "Show bash-guard policy locations and current mode.",
    handler: async (_args, ctx) => {
      const message = [
        `${EXTENSION_NAME} loaded`,
        `mode: ${isSubagent() ? "subagent/headless hard-block" : "interactive prompt"}`,
        `global policy: ~/.pi/agent/bash-guard-policy.json`,
        `local policy: ${resolve(ctx.cwd, ".pi/bash-guard-policy.json")}`,
      ].join("\n");
      if (ctx.hasUI) ctx.ui.notify(message, "info");
    },
  });

  const recentlyAborted = new Map<string, number>();
  const alwaysAllowed = new Set<string>();

  pi.on("tool_call", async (event, ctx) => {
    const policy = loadPolicy(ctx.cwd);
    const noUiAutoAllow = !ctx.hasUI && pi.getFlag("--bash-guard-auto-allow");

    if (isToolCallEventType("bash", event)) {
      const command = event.input.command;
      const allowKey = `bash:${command}`;
      if (alwaysAllowed.has(allowKey)) return;

      if (isSubagent() && !pi.getFlag("--bash-guard-headless-allow-catastrophic")) {
        for (const { pattern, reason } of HEADLESS_BLOCKED) {
          if (pattern.test(command)) {
            return {
              block: true,
              reason: `Blocked by bash-guard: ${reason}. This is a non-interactive subagent session. Propose a safer alternative or ask the parent agent to confirm with the user.`,
            };
          }
        }
      }

      const policyDecision = applyBashPolicy(command, policy);
      if (policyDecision?.action === "allow") return;
      if (policyDecision?.action === "block")
        return { block: true, reason: `Blocked by bash-guard: ${policyDecision.reason}` };

      const risk = mergeRisk(analyzeBashCommand(command), policyDecision?.risk ?? null);
      if (!risk) return;
      if (noUiAutoAllow) return;

      const now = Date.now();
      const lastAbort = recentlyAborted.get(allowKey);
      if (lastAbort && now - lastAbort < ABORT_REMEMBER_MS) {
        return {
          block: true,
          reason: "Blocked by bash-guard: command was already aborted recently. Do not retry the same command.",
        };
      }

      const choice = await promptRunOrAbort(ctx, "Potentially destructive bash command", `Command:\n${command}`, risk);
      if (choice === "run") return;
      if (choice === "always_allow") {
        alwaysAllowed.add(allowKey);
        return;
      }
      recentlyAborted.set(allowKey, now);
      return {
        block: true,
        reason: "Blocked by user via bash-guard. Ask for confirmation or propose a safer alternative.",
      };
    }

    const pathTool = isToolCallEventType("read", event)
      ? { tool: "read" as const, path: event.input.path }
      : isToolCallEventType("write", event)
        ? { tool: "write" as const, path: event.input.path }
        : isToolCallEventType("edit", event)
          ? { tool: "edit" as const, path: event.input.path }
          : null;

    if (!pathTool) return;
    const decision = applyPathPolicy(pathTool.tool, pathTool.path, ctx.cwd, policy);
    if (!decision || decision.action === "allow") return;
    if (decision.action === "block") return { block: true, reason: `Blocked by bash-guard: ${decision.reason}` };
    if (noUiAutoAllow) return;

    const key = `${pathTool.tool}:${pathTool.path}`;
    if (alwaysAllowed.has(key)) return;

    const now = Date.now();
    const lastAbort = recentlyAborted.get(key);
    if (lastAbort && now - lastAbort < ABORT_REMEMBER_MS) {
      return {
        block: true,
        reason: "Blocked by bash-guard: tool call was already aborted recently. Do not retry the same operation.",
      };
    }

    const choice = await promptRunOrAbort(
      ctx,
      "Sensitive file tool call",
      `Tool: ${pathTool.tool}\nPath: ${pathTool.path}`,
      decision.risk ?? { severity: "medium", reasons: ["sensitive path"] },
    );
    if (choice === "run") return;
    if (choice === "always_allow") {
      alwaysAllowed.add(key);
      return;
    }
    recentlyAborted.set(key, now);
    return { block: true, reason: "Blocked by user via bash-guard sensitive file policy." };
  });
}
