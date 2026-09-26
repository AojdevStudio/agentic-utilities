#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { stdin as input, stdout as output } from "node:process";
import { createInterface, type Interface } from "node:readline/promises";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import {
  applyDecision,
  completeAction,
  DECISIONS,
  type Decision,
  formatQueue,
  type GitHubCall,
  type GitHubStar,
  type Ledger,
  mergeStars,
  normalizeRepo,
  parseActiveProjects,
  parseGitHubStars,
  pendingActions,
  renderMarkdown,
  reviewQueue,
  type StarRecord,
  starRepositories,
  summarizeLedger,
} from "./github-stars-lib.ts";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));
const DATA_DIR =
  process.env.STARS_DATA_DIR || join(process.env.XDG_STATE_HOME || join(homedir(), ".local", "state"), "stars");
const LEDGER_PATH = join(DATA_DIR, "ledger.json");
const REVIEW_PATH = join(DATA_DIR, "review.md");
const PROJECTS_PATH = process.env.STARS_PROJECTS_PATH;
const SKILLS_LOCK_PATH = process.env.STARS_SKILLS_LOCK_PATH;
const CODEX_CONFIG = process.env.STARS_CODEX_CONFIG || join(homedir(), ".codex", "config.toml");
const SESSION_ID = crypto.randomUUID();
const VERSION = (JSON.parse(readFileSync(join(PACKAGE_ROOT, "package.json"), "utf8")) as { version: string }).version;

type CliArgs = {
  command: string;
  positionals: string[];
  limit: number;
  json: boolean;
  help: boolean;
  version: boolean;
  note: string;
  project: string;
};

type SyncResult = { ledger: Ledger; total: number };

class DependencyError extends Error {}

function run(command: string, args: string[], allowFailure = false): string {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 50 * 1024 * 1024 });
  if (result.status !== 0 && !allowFailure) {
    throw new DependencyError(
      `${command} ${args.join(" ")} failed: ${result.stderr?.trim() || `exit ${result.status}`}`,
    );
  }
  return result.status === 0 ? result.stdout.trim() : "";
}

const ghCall: GitHubCall = (args) => {
  const result = spawnSync("gh", args, { encoding: "utf8" });
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr || result.error?.message || "",
  };
};

function activeProjects(): string[] {
  if (!PROJECTS_PATH) return [];
  if (!existsSync(PROJECTS_PATH)) throw new Error(`STARS_PROJECTS_PATH does not exist: ${PROJECTS_PATH}`);
  return parseActiveProjects(readFileSync(PROJECTS_PATH, "utf8"));
}

function loadLedger(): Ledger | null {
  if (!existsSync(LEDGER_PATH)) return null;
  return JSON.parse(readFileSync(LEDGER_PATH, "utf8")) as Ledger;
}

function saveLedger(ledger: Ledger): void {
  mkdirSync(DATA_DIR, { recursive: true });
  const write = (path: string, content: string) => {
    const temporary = `${path}.${process.pid}.${crypto.randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, content, "utf8");
      renameSync(temporary, path);
    } catch (error) {
      rmSync(temporary, { force: true });
      throw error;
    }
  };
  write(LEDGER_PATH, `${JSON.stringify(ledger, null, 2)}\n`);
  write(REVIEW_PATH, renderMarkdown(ledger, activeProjects()));
}

function walkCheckouts(dir: string, depth: number, found: Map<string, string[]>): void {
  if (depth > 7) return;
  let entries: Array<{ name: string; isDirectory(): boolean }>;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  if (entries.some((entry) => entry.name === ".git")) {
    const slug = normalizeRepo(run("git", ["-C", dir, "remote", "get-url", "origin"], true));
    if (slug) found.set(slug, [...(found.get(slug) ?? []), `local checkout: ${dir}`]);
    return;
  }
  const skip = new Set([".git", "node_modules", ".next", "target", "dist", "build", "vendor"]);
  for (const entry of entries) {
    if (entry.isDirectory() && !skip.has(entry.name)) walkCheckouts(join(dir, entry.name), depth + 1, found);
  }
}

function checkoutEvidence(): Map<string, string[]> {
  const found = new Map<string, string[]>();
  const roots = (process.env.STARS_CHECKOUT_ROOTS || "").split(delimiter).filter(Boolean);
  for (const root of roots) {
    if (!existsSync(root)) throw new Error(`STARS_CHECKOUT_ROOTS directory does not exist: ${root}`);
    walkCheckouts(root, 0, found);
  }
  const host = process.env.STARS_SSH_HOST;
  const remoteRoot = process.env.STARS_SSH_ROOT;
  if (host || remoteRoot) {
    if (
      !host ||
      !remoteRoot ||
      !/^[A-Za-z0-9_.@-]+$/.test(host) ||
      !remoteRoot.startsWith("/") ||
      /[^A-Za-z0-9_./ -]/.test(remoteRoot)
    ) {
      throw new Error("STARS_SSH_HOST and STARS_SSH_ROOT must be set together to a host and absolute path.");
    }
    const remote = spawnSync(
      "ssh",
      [
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        host,
        `for gitdir in '${remoteRoot}'/*/.git '${remoteRoot}'/*/*/.git; do [ -e "$gitdir" ] || continue; repo=\${gitdir%/.git}; origin=\$(git -C "$repo" remote get-url origin 2>/dev/null) || continue; printf "%s\t%s\n" "$origin" "$repo"; done`,
      ],
      { encoding: "utf8", timeout: 10000 },
    );
    if (remote.status !== 0)
      throw new DependencyError(
        `SSH checkout inventory failed: ${remote.stderr?.trim() || remote.error?.message || `exit ${remote.status}`}`,
      );
    for (const line of remote.stdout.trim().split("\n")) {
      const [origin, path] = line.split("\t");
      const slug = normalizeRepo(origin ?? "");
      if (slug && path) found.set(slug, [...(found.get(slug) ?? []), `SSH checkout: ${path}`]);
    }
  }
  return found;
}

function skillEvidence(): Map<string, string[]> {
  const found = new Map<string, string[]>();
  if (!SKILLS_LOCK_PATH) return found;
  if (!existsSync(SKILLS_LOCK_PATH)) throw new Error(`STARS_SKILLS_LOCK_PATH does not exist: ${SKILLS_LOCK_PATH}`);
  const lock = JSON.parse(readFileSync(SKILLS_LOCK_PATH, "utf8")) as {
    skills?: Record<string, { source?: string; sourceType?: string }>;
  };
  for (const [name, item] of Object.entries(lock.skills ?? {})) {
    if (item.sourceType !== "github" || !item.source) continue;
    const slug = item.source
      .replace(/^https?:\/\/github\.com\//, "")
      .replace(/\.git$/, "")
      .toLowerCase();
    found.set(slug, [...(found.get(slug) ?? []), `installed skill: ${name}`]);
  }
  return found;
}

function pluginEvidence(): Map<string, string[]> {
  const found = new Map<string, string[]>();
  if (!existsSync(CODEX_CONFIG)) {
    if (process.env.STARS_CODEX_CONFIG) throw new Error(`STARS_CODEX_CONFIG does not exist: ${CODEX_CONFIG}`);
    return found;
  }
  const config = readFileSync(CODEX_CONFIG, "utf8");
  const enabled = [...config.matchAll(/\[plugins\."[^"]+@([^"]+)"\]\s+enabled\s*=\s*true/g)].map((match) => match[1]);
  for (const market of enabled) {
    const escaped = market.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const block = config.match(new RegExp(`\\[marketplaces\\.${escaped}\\]([\\s\\S]*?)(?=\\n\\[|$)`))?.[1] ?? "";
    const source = block.match(/^source\s*=\s*"([^"]+)"/m)?.[1] ?? "";
    const slug = normalizeRepo(source);
    if (slug) found.set(slug, [`enabled Codex plugin: ${market}`]);
  }
  return found;
}

function combineEvidence(...maps: Map<string, string[]>[]): Map<string, string[]> {
  const combined = new Map<string, string[]>();
  for (const map of maps) {
    for (const [key, values] of map) combined.set(key, [...(combined.get(key) ?? []), ...values]);
  }
  return combined;
}

function fetchStars(): { user: string; stars: GitHubStar[] } {
  const user = run("gh", ["api", "user", "--jq", ".login"]);
  const raw = run("gh", [
    "api",
    "user/starred",
    "-H",
    "Accept: application/vnd.github.star+json",
    "-f",
    "per_page=100",
    "--method",
    "GET",
    "--paginate",
    "--slurp",
  ]);
  return { user, stars: parseGitHubStars(raw) };
}

function assertLedgerAccount(ledger: Ledger, user = run("gh", ["api", "user", "--jq", ".login"])): void {
  if (ledger.githubUser.toLowerCase() !== user.toLowerCase()) {
    throw new Error(
      `GitHub account mismatch: ledger belongs to ${ledger.githubUser}, but gh is authenticated as ${user}.`,
    );
  }
}

function sync(): SyncResult {
  const previous = loadLedger();
  const { user, stars } = fetchStars();
  if (previous) assertLedgerAccount(previous, user);
  const evidence = combineEvidence(checkoutEvidence(), skillEvidence(), pluginEvidence());
  const { ledger } = mergeStars(previous, user, stars, evidence);
  saveLedger(ledger);
  return { ledger, total: stars.length };
}

function setDecision(ledger: Ledger, fullName: string, decision: Decision, note = "", project = ""): void {
  const record = ledger.records[fullName.toLowerCase()];
  if (!record) throw new Error(`Unknown star: ${fullName}. Run sync first.`);
  if (decision === "project" && !project.trim()) throw new Error("The project decision requires --project NAME.");
  applyDecision(record, decision, note, project);
  saveLedger(ledger);
}

async function askDecision(
  rl: Interface,
  record: StarRecord,
): Promise<{ decision: Decision; note: string; project: string } | null> {
  const labels = [
    "Related to an active project",
    "Install or evaluate as a developer tool",
    "Extract a prompt, skill, or implementation pattern",
    "Reference or learn from it",
    "Keep starred; already useful",
    "Mark as an unstar candidate",
    "Ask me again in 30 days",
    "Other / write in",
  ];
  output.write(`\n${formatQueue([record], activeProjects())}\n`);
  labels.forEach((label, index) => {
    output.write(`  ${index + 1}. ${label}\n`);
  });
  const answer = Number.parseInt(await rl.question("Choice [1-8, Enter to stop]: "), 10);
  if (!answer) return null;
  if (answer < 1 || answer > DECISIONS.length) throw new Error("Choice must be 1 through 8.");
  const decision = DECISIONS[answer - 1];
  const project = decision === "project" ? await rl.question("Active project: ") : "";
  return { decision, project, note: await rl.question("Why / note (optional): ") };
}

async function review(limit: number): Promise<void> {
  const ledger = loadLedger() ?? sync().ledger;
  const queue = reviewQueue(ledger, limit, new Date().toISOString(), activeProjects());
  if (!queue.length) return void process.stdout.write("No stars are due for review.\n");
  const rl = createInterface({ input, output });
  try {
    for (const record of queue) {
      const answer = await askDecision(rl, record);
      if (!answer) break;
      setDecision(ledger, record.fullName, answer.decision, answer.note, answer.project);
    }
  } finally {
    rl.close();
  }
}

async function applyUnstars(): Promise<void> {
  const ledger = loadLedger();
  if (!ledger) throw new Error("No ledger. Run sync first.");
  const candidates = Object.values(ledger.records).filter((record) => record.status === "unstar-candidate");
  if (!candidates.length) return void process.stdout.write("No unstar candidates.\n");
  const rl = createInterface({ input, output });
  try {
    for (const record of candidates) {
      const answer = (await rl.question(`Unstar ${record.fullName}? [y/N] `)).trim().toLowerCase();
      if (answer !== "y" && answer !== "yes") continue;
      assertLedgerAccount(ledger);
      run("gh", ["api", "--method", "DELETE", `user/starred/${record.fullName}`]);
      Object.assign(record, { status: "gone", goneAt: new Date().toISOString() });
      saveLedger(ledger);
      output.write(`Unstarred ${record.fullName}.\n`);
    }
  } finally {
    rl.close();
  }
}

function parseCli(argv: string[]): CliArgs {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      limit: { type: "string", short: "n", default: "5" },
      json: { type: "boolean", short: "j", default: false },
      help: { type: "boolean", short: "h", default: false },
      version: { type: "boolean", short: "v", default: false },
      note: { type: "string", default: "" },
      project: { type: "string", default: "" },
    },
  });
  if (!/^[1-9]\d*$/.test(values.limit)) throw new Error("--limit must be a positive integer.");
  const limit = Number(values.limit);
  if (!Number.isSafeInteger(limit)) throw new Error("--limit must be a safe positive integer.");
  return { ...values, command: positionals.shift() ?? "status", positionals, limit };
}

function printStatus(ledger: Ledger, json: boolean): void {
  const data = summarizeLedger(ledger);
  if (json) return void process.stdout.write(`${JSON.stringify(data, null, 2)}\n`);
  const projects = activeProjects();
  const next = reviewQueue(ledger, 3, new Date().toISOString(), projects);
  process.stdout.write(
    `GitHub Stars\n${data.current} current · ${data.review} to review · ${data.resolved} resolved · ${data.pendingActions} pending actions · ${data.unstarCandidates} unstar candidates\nAccount: ${data.account}\nLast sync: ${data.lastSync}\n\nNext up\n${formatQueue(next, projects)}`,
  );
}

function selfTest(): void {
  if (normalizeRepo("git@github.com:owner/repo.git") !== "owner/repo") throw new Error("SSH URL parse failed");
  if (parseCli(["queue", "--limit", "3"]).limit !== 3) throw new Error("CLI limit normalization failed");
  let rejectedInvalidLimit = false;
  try {
    parseCli(["queue", "--limit", "3extra"]);
  } catch {
    rejectedInvalidLimit = true;
  }
  if (!rejectedInvalidLimit) throw new Error("CLI accepted an invalid limit");
  const records = {
    a: { fullName: "new", starredAt: "2026-01-03", status: "unreviewed" },
    b: { fullName: "old", starredAt: "2020-01-01", status: "unreviewed" },
    c: { fullName: "middle", starredAt: "2023-01-01", status: "unreviewed" },
  };
  const names = reviewQueue({ records } as unknown as Ledger, 3)
    .map((record) => record.fullName)
    .join(",");
  if (names !== "new,old,middle") throw new Error(`mixed-age queue failed: ${names}`);
  process.stdout.write("Self-test passed.\n");
}

function help(): void {
  process.stdout.write(
    `stars ${VERSION}\n\nUsage:\n  stars [status] [--json]\n  stars star OWNER/REPO [OWNER/REPO...] [--json]\n  stars sync [--json]\n  stars review [-n 5]\n  stars queue [-n 5] [--json]\n  stars actions [--json]\n  stars done OWNER/REPO\n  stars decide OWNER/REPO DECISION [--project NAME] [--note TEXT]\n  stars unstar\n\nCommands:\n  status    Dashboard and next three reviews\n  star      Verify and star exact GitHub repository URLs or owner/repo refs, then sync\n  sync      Refresh GitHub and checkout evidence\n  review    Interactive multiple-choice review\n  queue     Show prioritized recent and old stars\n  actions   Show decisions with unfinished follow-up work\n  done      Mark a pending action complete\n  decide    Record a user or LLM decision\n  unstar    Confirm and remove marked candidates one by one\n\nDecisions: ${DECISIONS.join(", ")}\n`,
  );
}

async function main(): Promise<void> {
  const args = parseCli(process.argv.slice(2));
  if (args.version) return void process.stdout.write(`stars ${VERSION}\n`);
  if (args.help || args.command === "help") return help();
  if (args.command === "star") {
    if (!args.positionals.length) throw new Error("Usage: stars star OWNER/REPO [OWNER/REPO...] [--json]");
    const ledger = loadLedger();
    if (ledger) assertLedgerAccount(ledger);
    const results = starRepositories(args.positionals, ghCall, sync);
    if (args.json) process.stdout.write(`${JSON.stringify({ results }, null, 2)}\n`);
    else
      for (const result of results) {
        const workflow =
          result.workflow.status === "failed"
            ? `workflow failed: ${result.workflow.error}`
            : result.workflow.status === "synced"
              ? "workflow synced"
              : "workflow skipped";
        process.stdout.write(
          `${result.url ?? result.requested}: ${result.status}${result.error ? ` (${result.error})` : ""}; ${workflow}\n`,
        );
      }
    if (results.some((result) => result.status === "failed" || result.workflow.status === "failed"))
      process.exitCode = 1;
  } else if (args.command === "sync") {
    const result = sync();
    if (args.json)
      process.stdout.write(`${JSON.stringify({ total: result.total, ...summarizeLedger(result.ledger) }, null, 2)}\n`);
    else process.stdout.write(`Synced ${result.total} stars.\n${REVIEW_PATH}\n`);
  } else if (args.command === "review") await review(args.limit);
  else if (args.command === "queue" || args.command === "next") {
    const ledger = loadLedger() ?? sync().ledger;
    const projects = activeProjects();
    const stars = reviewQueue(ledger, args.limit, new Date().toISOString(), projects);
    if (args.json || args.command === "next")
      process.stdout.write(`${JSON.stringify({ activeProjects: activeProjects(), stars }, null, 2)}\n`);
    else process.stdout.write(formatQueue(stars, projects));
  } else if (args.command === "actions") {
    const ledger = loadLedger();
    if (!ledger) throw new Error("No ledger. Run sync first.");
    const actions = pendingActions(ledger);
    if (args.json) process.stdout.write(`${JSON.stringify(actions, null, 2)}\n`);
    else
      process.stdout.write(
        actions.length
          ? `${actions.map((record) => `${record.fullName} · ${record.decision}${record.project ? ` · ${record.project}` : ""}${record.note ? ` · ${record.note.trim()}` : ""}`).join("\n")}\n`
          : "No pending actions.\n",
      );
  } else if (args.command === "done") {
    const ledger = loadLedger();
    const name = args.positionals[0]?.toLowerCase();
    if (!ledger || !name) throw new Error("Run sync, then: done OWNER/REPO");
    const record = ledger.records[name];
    if (!record || !pendingActions(ledger).includes(record)) throw new Error(`${name} has no pending action.`);
    completeAction(record);
    saveLedger(ledger);
    process.stdout.write(`Completed ${record.fullName}: ${record.decision}.\n`);
  } else if (args.command === "decide") {
    const ledger = loadLedger();
    const [fullName, decision] = args.positionals as [string, Decision];
    if (!ledger || !fullName || !DECISIONS.includes(decision as (typeof DECISIONS)[number]))
      throw new Error("Run sync, then: decide OWNER/REPO DECISION");
    setDecision(ledger, fullName, decision, args.note, args.project);
    process.stdout.write(`Recorded ${fullName}: ${decision}.\n`);
  } else if (args.command === "unstar" || args.command === "unstars") await applyUnstars();
  else if (args.command === "status") {
    const ledger = loadLedger();
    if (!ledger) throw new Error("No baseline yet. Run: stars sync");
    printStatus(ledger, args.json);
  } else if (args.command === "self-test") selfTest();
  else throw new Error(`Unknown command: ${args.command}. Run stars --help.`);
}

main().catch((error) => {
  process.stderr.write(
    `${JSON.stringify({ ts: new Date().toISOString(), level: "error", sessionId: SESSION_ID, msg: "github-stars.failed", error: error instanceof Error ? error.message : String(error) })}\n`,
  );
  process.exitCode = 1;
});
