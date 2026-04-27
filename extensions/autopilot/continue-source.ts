import * as crypto from "node:crypto";
import * as fs from "node:fs";
import * as fsp from "node:fs/promises";
import * as path from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

import { DEFAULT_REPO_AUTOPILOT_PREFERENCES, loadRepoAutopilotPreferences, type RepoAutopilotPreferences, type VerificationProfile } from "./prefs.ts";

type PlanSource = {
  kind: "plan";
  reference: string;
  title: string;
  body: string;
  path: string;
};

type GitHubSource = {
  kind: "github";
  reference: string;
  title: string;
  body: string;
  url: string;
  number: number;
  labels: string[];
  comments: Array<{ author: string; body: string }>;
};

type LinearSource = {
  kind: "linear";
  reference: string;
  title: string;
  body: string;
  url: string;
  issueId: string;
  comments: Array<{ author: string; body: string }>;
};

export type ContinueSource = PlanSource | GitHubSource | LinearSource;

export async function buildContinueManifest(pi: ExtensionAPI, repoCwd: string, sourceInput: string): Promise<{ manifestPath: string; source: ContinueSource }> {
  const source = await resolveContinueSource(pi, repoCwd, sourceInput);
  const branch = await getCurrentGitBranch(pi, repoCwd);
  const preferences = loadRepoAutopilotPreferences(repoCwd);
  const manifestDir = path.join(repoCwd, ".pi", "autopilot");
  ensureDir(manifestDir);

  const manifestId = buildManifestId(source);
  const manifestPath = path.join(manifestDir, `${manifestId}.md`);
  const checks = buildChecksForSource(source, preferences.verificationProfile, repoCwd);
  const markdown = buildManifestMarkdown({ source, branch, checks, preferences });

  await fsp.writeFile(manifestPath, markdown, "utf8");
  return { manifestPath, source };
}

function ensureDir(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
}

function fileExists(p: string): boolean {
  try {
    return fs.existsSync(p);
  } catch {
    return false;
  }
}

function slugify(input: string): string {
  return input
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function shortHash(input: string): string {
  return crypto.createHash("sha256").update(input).digest("hex").slice(0, 10);
}

function yamlScalar(value: string): string {
  return JSON.stringify(value);
}

function buildManifestId(source: ContinueSource): string {
  if (source.kind === "plan") {
    const base = path.basename(source.path).replace(/\.[^.]+$/, "");
    const slug = slugify(base) || "autopilot-plan";
    return `${slug.slice(0, 60)}-${shortHash(source.path)}`;
  }

  const slug = slugify(`${source.kind}-${source.reference}-${source.title}`) || `autopilot-${source.kind}`;
  return `${slug.slice(0, 60)}-${shortHash(source.reference)}`;
}

function buildManifestMarkdown(args: { source: ContinueSource; branch: string; checks: string[]; preferences: RepoAutopilotPreferences }): string {
  const { source, branch, checks, preferences } = args;
  const allowPaths = preferences.allowPaths.length ? preferences.allowPaths : DEFAULT_REPO_AUTOPILOT_PREFERENCES.allowPaths;
  const denyPaths = preferences.denyPaths.length ? preferences.denyPaths : DEFAULT_REPO_AUTOPILOT_PREFERENCES.denyPaths;

  const frontmatterLines = [
    "---",
    `id: ${yamlScalar(buildManifestId(source))}`,
    `branch: ${yamlScalar(branch)}`,
    `source_kind: ${yamlScalar(source.kind)}`,
    `source_ref: ${yamlScalar(source.reference)}`,
    `source_priority: [${preferences.sourcePriority.join(", ")}]`,
    `verification_profile: ${yamlScalar(preferences.verificationProfile)}`,
  ];

  if (source.kind !== "plan") {
    frontmatterLines.push(`source_url: ${yamlScalar(source.url)}`);
    frontmatterLines.push(`source_title: ${yamlScalar(source.title)}`);
  } else {
    frontmatterLines.push(`source_path: ${yamlScalar(source.path)}`);
    frontmatterLines.push(`source_title: ${yamlScalar(source.title)}`);
  }

  frontmatterLines.push(
    "authority:",
    "  commit: false",
    "  push: true",
    "  pr: true",
    "  merge: false",
    "scope:",
  );

  if (source.kind === "github") {
    frontmatterLines.push(`  gh_issue_ids: [${source.number}]`);
    frontmatterLines.push("  todo_ids: []");
    frontmatterLines.push("  linear_issue_ids: []");
  } else if (source.kind === "linear") {
    frontmatterLines.push("  gh_issue_ids: []");
    frontmatterLines.push("  todo_ids: []");
    frontmatterLines.push(`  linear_issue_ids: [${yamlScalar(source.issueId)}]`);
  } else {
    frontmatterLines.push("  gh_issue_ids: []");
    frontmatterLines.push("  todo_ids: []");
    frontmatterLines.push("  linear_issue_ids: []");
  }

  frontmatterLines.push(
    "paths:",
    "  allow:",
    ...allowPaths.map((p) => `    - ${p}`),
    "  related: []",
    "  deny:",
    ...denyPaths.map((p) => `    - ${p}`),
    "checks:",
    ...checks.map((check) => `  - type: command\n    run: ${check}`),
    "verify:",
    "  - all_required_checks_passed",
    "acceptance:",
    source.kind === "plan"
      ? "  - complete the plan"
      : "  - resolve the source issue",
    "  - required checks pass",
    "stop_when:",
    source.kind === "plan" ? "  - plan work is complete" : "  - source issue is resolved",
    "---",
  );

  const body = source.kind === "plan"
    ? buildPlanBody(source)
    : buildIssueBody(source);

  return `${frontmatterLines.join("\n")}\n\n${body}\n`;
}

function buildPlanBody(source: PlanSource): string {
  return [
    "## Source plan",
    `- path: ${source.path}`,
    `- reference: ${source.reference}`,
    "",
    "## Plan content",
    source.body.trim() || "_Empty plan file._",
    "",
    "## Autopilot instructions",
    "- Follow the plan faithfully and keep the smallest coherent change set.",
    "- Prefer verification over guessing.",
    "- If the plan is incomplete, inspect nearby docs and source before editing.",
  ].join("\n");
}

function buildIssueBody(source: GitHubSource | LinearSource): string {
  const commentSection = source.comments.length
    ? [
        "## Comments",
        ...source.comments.slice(0, 5).map((comment, index) => [
          `### Comment ${index + 1}`,
          `- author: ${comment.author || "unknown"}`,
          "",
          comment.body.trim() || "_Empty comment._",
        ].join("\n")),
      ]
    : [];

  const extraCommentNote = source.comments.length > 5
    ? [``, `... and ${source.comments.length - 5} more comments.`]
    : [];

  const labelSection = source.kind === "github" && source.labels.length
    ? [`- labels: ${source.labels.join(", ")}`]
    : [];

  return [
    "## Source issue",
    `- tracker: ${source.kind === "github" ? "GitHub" : "Linear"}`,
    `- reference: ${source.reference}`,
    `- url: ${source.url}`,
    `- title: ${source.title}`,
    ...labelSection,
    "",
    "## Issue body",
    source.body.trim() || "_No issue body provided._",
    ...commentSection,
    ...extraCommentNote,
    "",
    "## Autopilot instructions",
    "- Treat the issue body as the contract and keep changes narrowly scoped.",
    "- Find the relevant files first, then implement the smallest correct fix.",
    "- If the issue is ambiguous, inspect adjacent docs or previous work before editing.",
  ].join("\n");
}

function buildChecksForSource(source: ContinueSource, profile: VerificationProfile, repoCwd: string): string[] {
  const text = `${source.title}\n${source.body}\n${source.kind === "plan" ? "" : source.comments.map((c) => c.body).join("\n")}`.toLowerCase();
  const checks = new Set<string>(["git diff --check"]);

  if (isHomelabRepo(repoCwd)) {
    addHomelabChecks(checks, text, repoCwd, profile);
    return [...checks];
  }

  addGenericChecks(checks, text, repoCwd, profile);
  return [...checks];
}

function addGenericChecks(checks: Set<string>, text: string, repoCwd: string, profile: VerificationProfile): void {
  const strict = profile === "strict";
  const conservative = profile === "conservative";

  if (/\b(tauri|src-tauri|cargo|rust)\b/.test(text) && fileExists(path.join(repoCwd, "app", "src-tauri"))) {
    checks.add("cd app/src-tauri && cargo check");
  }

  if (/\b(snaptrade|sqlite|drizzle|database|portfolio|sync|income|transactions)\b/.test(text) && fileExists(path.join(repoCwd, "packages", "data"))) {
    checks.add("cd packages/data && bun test");
  }

  if ((/\b(server|api|route|elysia|context injector|context)\b/.test(text) || strict) && fileExists(path.join(repoCwd, "packages", "core"))) {
    checks.add("cd packages/core && bun test src/server-contract.test.ts");
  }

  if ((/\b(ui|react|component|view|frontend|app)\b/.test(text) || strict) && fileExists(path.join(repoCwd, "app"))) {
    checks.add("cd app && bun run typecheck");
  }

  if ((text.includes("strategy composer") || (text.includes("strategy") && text.includes("dry-run")) || strict) && fileExists(path.join(repoCwd, "app"))) {
    checks.add("cd app && bun run test src/features/strategy/__tests__/hooks.test.ts src/features/strategy/__tests__/StrategyComposerView.test.tsx");
  }

  if (strict && hasPackageScript(repoCwd, "typecheck")) {
    checks.add("npm run typecheck");
  }

  if (conservative) {
    for (const check of [...checks]) {
      if (check !== "git diff --check" && !/cargo check|packages\/data|server-contract|typecheck/.test(check)) {
        checks.delete(check);
      }
    }
  }
}

function addHomelabChecks(checks: Set<string>, text: string, repoCwd: string, profile: VerificationProfile): void {
  const strict = profile === "strict";
  const conservative = profile === "conservative";

  const mention = (...needles: string[]) => needles.some((needle) => text.includes(needle));
  const addComposeCheck = (relativePath: string) => {
    if (fileExists(path.join(repoCwd, relativePath))) {
      checks.add(`docker compose -f ${relativePath} config`);
    }
  };

  if (mention("nas/docker-compose", "nas/traefik", "traefik", "portainer", "tailscale-router") || strict) {
    addComposeCheck("nas/docker-compose.yml");
  }

  if (mention("nas/media-server", "jellyfin", "plex", "gluetun", "qbittorrent", "radarr", "sonarr", "prowlarr", "overseerr") || strict) {
    addComposeCheck("nas/media-server/docker-compose.yml");
    if (!conservative) addComposeCheck("nas/media-server/docker-compose.openvpn.yml");
    addComposeCheck("mac-mini-server/plex/docker-compose.yml");
  }

  if (mention("immich", "nas/immich") || strict) {
    addComposeCheck("nas/immich/docker-compose.yml");
  }

  if (mention("proxmox/docker-lxc", "grafana", "prometheus", "cloudflared", "cloudflare tunnel", "homepage", "docker-lxc") || strict) {
    addComposeCheck("proxmox/docker-lxc/docker-compose.yml");
  }

  if (mention("proxmox/media-automation", "media-automation", "bazarr", "readarr", "sabnzbd") || strict) {
    addComposeCheck("proxmox/media-automation/docker-compose.yml");
  }

  if (mention("prometheus", "alert rule", "recording rule", "prometheus.yml", "homelab-health.yml") || strict) {
    const promDir = path.join(repoCwd, "proxmox", "docker-lxc", "prometheus");
    if (fileExists(path.join(promDir, "prometheus.yml"))) {
      checks.add('docker run --rm -v "$PWD/proxmox/docker-lxc/prometheus:/etc/prometheus:ro" --entrypoint promtool prom/prometheus:latest check config /etc/prometheus/prometheus.yml');
    }
    if (fileExists(path.join(promDir, "rules", "homelab-health.yml")) && !conservative) {
      checks.add('docker run --rm -v "$PWD/proxmox/docker-lxc/prometheus:/etc/prometheus:ro" --entrypoint promtool prom/prometheus:latest check rules /etc/prometheus/rules/homelab-health.yml');
    }
  }

  if (mention("proxmox/homepage/phi", " phi", "homepage generator", "services.yaml", "widgets.yaml", "settings.yaml", "traefik-phi.yml") || strict) {
    if (fileExists(path.join(repoCwd, "proxmox", "homepage", "phi", "go.mod"))) {
      checks.add("cd proxmox/homepage/phi && go test ./...");
    }
  }

  if (mention("dashboard/", "next.js", "react", "dashboard") || strict) {
    if (fileExists(path.join(repoCwd, "dashboard", "package.json")) && hasPackageScript(path.join(repoCwd, "dashboard"), "lint")) {
      checks.add("cd dashboard && npm run lint");
    }
  }
}

function isHomelabRepo(repoCwd: string): boolean {
  return fileExists(path.join(repoCwd, "AGENTS.md"))
    && fileExists(path.join(repoCwd, "nas"))
    && fileExists(path.join(repoCwd, "proxmox"));
}

function hasPackageScript(dir: string, script: string): boolean {
  try {
    const packageJson = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8")) as { scripts?: Record<string, string> };
    return typeof packageJson.scripts?.[script] === "string";
  } catch {
    return false;
  }
}

async function resolveContinueSource(pi: ExtensionAPI, repoCwd: string, sourceInput: string): Promise<ContinueSource> {
  const normalized = sourceInput.trim();
  if (!normalized) {
    throw new Error("missing source argument. Expected a plan file path, GitHub issue reference, or Linear issue identifier");
  }

  const planPath = resolvePlanPath(repoCwd, normalized);
  if (planPath) {
    const body = await fsp.readFile(planPath, "utf8");
    return {
      kind: "plan",
      reference: planPath,
      title: path.basename(planPath),
      body,
      path: planPath,
    };
  }

  const githubRef = await parseGitHubSource(pi, repoCwd, normalized);
  if (githubRef) {
    return await loadGitHubIssue(pi, githubRef);
  }

  const linearRef = parseLinearSource(normalized);
  if (linearRef) {
    return await loadLinearIssue(pi, linearRef);
  }

  throw new Error(`Unsupported source: ${sourceInput}`);
}

function resolvePlanPath(repoCwd: string, input: string): string | null {
  const absolute = path.isAbsolute(input) ? input : path.resolve(repoCwd, input);
  if (fileExists(absolute)) {
    const stats = fs.statSync(absolute);
    if (stats.isFile()) return absolute;
  }
  return null;
}

async function parseGitHubSource(
  pi: ExtensionAPI,
  repoCwd: string,
  input: string,
): Promise<{ owner: string; repo: string; number: number; reference: string; url?: string } | null> {
  const refMatch = input.match(/^([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)#(\d+)$/);
  if (refMatch) {
    return {
      owner: refMatch[1],
      repo: refMatch[2],
      number: Number.parseInt(refMatch[3] ?? "", 10),
      reference: `${refMatch[1]}/${refMatch[2]}#${refMatch[3]}`,
    };
  }

  const urlMatch = input.match(/^(?:https?:\/\/)?github\.com\/([^/]+)\/([^/]+)\/issues\/(\d+)(?:[/?#].*)?$/i);
  if (urlMatch) {
    return {
      owner: urlMatch[1],
      repo: urlMatch[2],
      number: Number.parseInt(urlMatch[3] ?? "", 10),
      reference: `${urlMatch[1]}/${urlMatch[2]}#${urlMatch[3]}`,
      url: input.startsWith("http") ? input : `https://${input}`,
    };
  }

  const localIssueMatch = input.match(/^(?:gh\s+issue|github\s+issue|issue)\s+#?(\d+)$/i) ?? input.match(/^#(\d+)$/);
  if (!localIssueMatch) {
    return null;
  }

  const repoRef = await resolveCurrentGitHubRepo(pi, repoCwd);
  if (!repoRef) {
    throw new Error(`Could not resolve current GitHub repo for ${repoCwd}. Use owner/repo#123 or a full GitHub issue URL.`);
  }

  const issueNumber = Number.parseInt(localIssueMatch[1] ?? "", 10);
  return {
    owner: repoRef.owner,
    repo: repoRef.repo,
    number: issueNumber,
    reference: `${repoRef.owner}/${repoRef.repo}#${issueNumber}`,
    url: `https://github.com/${repoRef.owner}/${repoRef.repo}/issues/${issueNumber}`,
  };
}

async function resolveCurrentGitHubRepo(pi: ExtensionAPI, repoCwd: string): Promise<{ owner: string; repo: string } | null> {
  const remoteResult = await pi.exec(
    "bash",
    ["-lc", `cd ${shellQuote(repoCwd)} && git config --get remote.origin.url`],
    { timeout: 10_000 } as any,
  );
  const remoteUrl = String(remoteResult.stdout ?? "").trim();
  const parsedRemote = parseGitHubRemoteUrl(remoteUrl);
  if (parsedRemote) {
    return parsedRemote;
  }

  const ghResult = await pi.exec(
    "bash",
    ["-lc", `cd ${shellQuote(repoCwd)} && gh repo view --json owner,name`],
    { timeout: 20_000 } as any,
  );
  if (ghResult.code !== 0) {
    return null;
  }

  const parsed = safeJsonParse(ghResult.stdout);
  if (!parsed || typeof parsed !== "object") {
    return null;
  }

  const raw = parsed as Record<string, any>;
  const owner = stringValue(raw.owner?.login, stringValue(raw.owner?.name, ""));
  const repo = stringValue(raw.name, "");
  if (!owner || !repo) {
    return null;
  }

  return { owner, repo };
}

function parseGitHubRemoteUrl(remoteUrl: string): { owner: string; repo: string } | null {
  const trimmed = remoteUrl.trim();
  if (!trimmed) return null;

  const match = trimmed.match(/github\.com[:/]([^/]+)\/([^/]+?)(?:\.git)?$/i);
  if (!match) {
    return null;
  }

  return {
    owner: match[1],
    repo: match[2],
  };
}

function parseLinearSource(input: string): { issueId: string; reference: string; url?: string } | null {
  const refMatch = input.match(/^([A-Za-z][A-Za-z0-9]+-\d+)$/);
  if (refMatch) {
    return {
      issueId: refMatch[1],
      reference: refMatch[1],
    };
  }

  const urlMatch = input.match(/^(?:https?:\/\/)?linear\.app\/[^/]+\/issue\/([A-Za-z][A-Za-z0-9]+-\d+)(?:\/|$)/i);
  if (urlMatch) {
    return {
      issueId: urlMatch[1],
      reference: urlMatch[1],
      url: input.startsWith("http") ? input : `https://${input}`,
    };
  }

  return null;
}

async function loadGitHubIssue(pi: ExtensionAPI, ref: { owner: string; repo: string; number: number; reference: string; url?: string }): Promise<GitHubSource> {
  const result = await pi.exec(
    "gh",
    [
      "issue",
      "view",
      String(ref.number),
      "--repo",
      `${ref.owner}/${ref.repo}`,
      "--comments",
      "--json",
      "title,body,number,url,labels,comments,state,author",
    ],
    { timeout: 120_000 } as any,
  );

  if (result.code !== 0) {
    throw new Error(`gh issue view failed for ${ref.reference}: ${String(result.stderr || result.stdout || `exit ${result.code}`)}`);
  }

  const parsed = safeJsonParse(result.stdout);
  if (!parsed || typeof parsed !== "object") {
    throw new Error(`gh issue view returned unexpected output for ${ref.reference}`);
  }

  const raw = parsed as Record<string, any>;
  return {
    kind: "github",
    reference: ref.reference,
    title: stringValue(raw.title, `GitHub issue ${ref.reference}`),
    body: stringValue(raw.body, ""),
    url: stringValue(raw.url, ref.url ?? `https://github.com/${ref.owner}/${ref.repo}/issues/${ref.number}`),
    number: ref.number,
    labels: normalizeLabels(raw.labels),
    comments: normalizeComments(raw.comments),
  };
}

async function loadLinearIssue(pi: ExtensionAPI, ref: { issueId: string; reference: string; url?: string }): Promise<LinearSource> {
  const result = await pi.exec(
    "linearis",
    ["issues", "read", ref.issueId],
    { timeout: 120_000 } as any,
  );

  if (result.code !== 0) {
    throw new Error(`linearis issues read failed for ${ref.reference}: ${String(result.stderr || result.stdout || `exit ${result.code}`)}`);
  }

  const parsed = safeJsonParse(result.stdout);
  const raw = parsed && typeof parsed === "object" ? (parsed as Record<string, any>) : undefined;

  if (raw) {
    const url = stringValue(raw.url, ref.url ?? "");
    return {
      kind: "linear",
      reference: stringValue(raw.identifier, ref.reference),
      title: stringValue(raw.title, `Linear issue ${ref.reference}`),
      body: stringValue(raw.description, stringValue(raw.body, stringValue(raw.content, ""))),
      url: url || ref.url || `https://linear.app`,
      issueId: stringValue(raw.id, ref.issueId),
      comments: normalizeComments(raw.comments),
    };
  }

  return {
    kind: "linear",
    reference: ref.reference,
    title: `Linear issue ${ref.reference}`,
    body: String(result.stdout ?? ""),
    url: ref.url ?? "https://linear.app",
    issueId: ref.issueId,
    comments: [],
  };
}

async function getCurrentGitBranch(pi: ExtensionAPI, repoCwd: string): Promise<string> {
  const result = await pi.exec(
    "bash",
    ["-lc", `cd ${shellQuote(repoCwd)} && git rev-parse --abbrev-ref HEAD`],
    { timeout: 10_000 } as any,
  );
  if (result.code !== 0) return "";
  return String(result.stdout ?? "").trim();
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'"'"'`)}'`;
}

function safeJsonParse(input: unknown): unknown | null {
  const text = String(input ?? "").trim();
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function normalizeLabels(raw: unknown): string[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => {
      if (typeof item === "string") return item;
      if (item && typeof item === "object") {
        const record = item as Record<string, any>;
        return stringValue(record.name, stringValue(record.title, stringValue(record.id, "")));
      }
      return "";
    })
    .filter((item) => item.length > 0);
}

function normalizeComments(raw: unknown): Array<{ author: string; body: string }> {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => {
      if (typeof item === "string") {
        return { author: "", body: item };
      }
      if (!item || typeof item !== "object") return null;
      const record = item as Record<string, any>;
      const author = stringValue(
        record.author?.login,
        stringValue(record.author?.name, stringValue(record.user?.name, stringValue(record.createdBy?.name, stringValue(record.creator?.name, "")))),
      );
      const body = stringValue(record.body, stringValue(record.content, stringValue(record.text, "")));
      if (!body.trim()) return null;
      return { author, body };
    })
    .filter((item): item is { author: string; body: string } => item !== null);
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value : fallback;
}
