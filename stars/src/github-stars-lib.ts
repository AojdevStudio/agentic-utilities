export const DECISIONS = ["project", "install", "extract", "reference", "keep", "unstar", "later", "other"] as const;
export const ACTION_DECISIONS = ["project", "install", "extract"] as const;
export type Decision = (typeof DECISIONS)[number] | "used";
export type ReviewStatus = "unreviewed" | "resolved" | "unstar-candidate" | "snoozed" | "gone";

export interface GitHubStar {
  starred_at: string;
  repo: {
    archived: boolean;
    description: string | null;
    full_name: string;
    html_url: string;
    language: string | null;
    pushed_at: string | null;
    stargazers_count: number;
    topics?: string[];
  };
}

export interface StarRecord {
  fullName: string;
  url: string;
  description: string;
  language: string | null;
  topics: string[];
  stars: number;
  archived: boolean;
  pushedAt: string | null;
  starredAt: string;
  firstSeenAt: string;
  lastSeenAt: string;
  status: ReviewStatus;
  decision?: Decision;
  note?: string;
  project?: string;
  evidence: string[];
  reviewedAt?: string;
  reviewAfter?: string;
  goneAt?: string;
  actionCompletedAt?: string;
}

export interface Ledger {
  version: 1;
  githubUser: string;
  baselineAt: string;
  syncedAt: string;
  records: Record<string, StarRecord>;
}

export interface LedgerSummary {
  account: string;
  current: number;
  review: number;
  resolved: number;
  unstarCandidates: number;
  snoozed: number;
  gone: number;
  pendingActions: number;
  lastSync: string;
}

export class ValidationError extends Error {}

export type StarActionResult = {
  requested: string;
  fullName?: string;
  url?: string;
  status: "starred" | "already-starred" | "failed";
  error?: string;
  workflow: { status: "synced" | "failed" | "skipped"; error?: string };
};

export type GitHubCall = (args: string[]) => { status: number; stdout: string; stderr: string };

/** Accept only a repository's exact GitHub URL or an explicit owner/repo path. */
export function parseStarRef(ref: string): string {
  const value = ref.trim();
  let path = value;
  let cloneUrl = false;
  if (value.includes("://")) {
    let url: URL;
    try {
      url = new URL(value);
    } catch {
      throw new ValidationError(`Invalid GitHub repository URL: ${ref}`);
    }
    if (
      url.protocol !== "https:" ||
      url.hostname.toLowerCase() !== "github.com" ||
      url.username ||
      url.password ||
      url.port ||
      url.search ||
      url.hash
    ) {
      throw new ValidationError(`Expected an exact github.com repository URL: ${ref}`);
    }
    path = url.pathname.replace(/^\//, "").replace(/\/$/, "");
    cloneUrl = true;
  } else if (value.startsWith("git@github.com:")) {
    path = value.slice("git@github.com:".length);
    cloneUrl = true;
  }
  if (cloneUrl && path.endsWith(".git")) path = path.slice(0, -4);
  if (!/^[A-Za-z0-9-]+\/[A-Za-z0-9._-]+$/.test(path) || path.endsWith("/.") || path.endsWith("/..")) {
    throw new ValidationError(`Expected OWNER/REPO or an exact GitHub repository URL: ${ref}`);
  }
  return path;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Stars explicit repository references and syncs the normal ledger workflow once per batch. */
export function starRepositories(refs: string[], gh: GitHubCall, syncWorkflow: () => void): StarActionResult[] {
  const results: StarActionResult[] = [];
  for (const requested of refs) {
    const result: StarActionResult = { requested, status: "failed", workflow: { status: "skipped" } };
    results.push(result);
    try {
      const ref = parseStarRef(requested);
      const resolved = gh(["api", `repos/${ref}`]);
      if (resolved.status !== 0)
        throw new Error(`Repository lookup failed: ${resolved.stderr.trim() || `exit ${resolved.status}`}`);
      const repo: unknown = JSON.parse(resolved.stdout);
      if (
        !repo ||
        typeof repo !== "object" ||
        !("full_name" in repo) ||
        !("html_url" in repo) ||
        typeof repo.full_name !== "string" ||
        typeof repo.html_url !== "string"
      ) {
        throw new ValidationError("GitHub repository lookup omitted canonical identity.");
      }
      const canonical = parseStarRef(repo.html_url);
      if (canonical.toLowerCase() !== repo.full_name.toLowerCase()) {
        throw new ValidationError("GitHub repository identity did not match its canonical URL.");
      }
      result.fullName = repo.full_name;
      result.url = repo.html_url;
      const state = gh(["api", `user/starred/${canonical}`, "-X", "GET", "--silent"]);
      if (state.status === 0) {
        result.status = "already-starred";
        continue;
      }
      if (!/\bHTTP 404\b/i.test(state.stderr)) {
        throw new Error(`Could not check starring state: ${state.stderr.trim() || `exit ${state.status}`}`);
      }
      const write = gh(["api", `user/starred/${canonical}`, "-X", "PUT", "--silent"]);
      if (write.status !== 0)
        throw new Error(`Could not star repository: ${write.stderr.trim() || `exit ${write.status}`}`);
      result.status = "starred";
    } catch (error) {
      result.error = errorText(error);
    }
  }
  if (results.some((result) => result.status !== "failed")) {
    try {
      syncWorkflow();
      for (const result of results) if (result.status !== "failed") result.workflow = { status: "synced" };
    } catch (error) {
      for (const result of results)
        if (result.status !== "failed") result.workflow = { status: "failed", error: errorText(error) };
    }
  }
  return results;
}

export function normalizeRepo(url: string): string | null {
  const match = url.trim().match(/github\.com[:/]([^/]+)\/([^/#]+?)(?:\.git)?$/i);
  return match ? `${match[1]}/${match[2]}`.toLowerCase() : null;
}

function isGitHubStar(value: unknown): value is GitHubStar {
  if (!value || typeof value !== "object") return false;
  const item = value as Partial<GitHubStar>;
  const repo = item.repo as Partial<GitHubStar["repo"]> | undefined;
  return (
    typeof item.starred_at === "string" &&
    typeof repo?.full_name === "string" &&
    typeof repo.html_url === "string" &&
    typeof repo.archived === "boolean" &&
    typeof repo.stargazers_count === "number"
  );
}

export function parseGitHubStars(raw: string): GitHubStar[] {
  let pages: unknown;
  try {
    pages = JSON.parse(raw);
  } catch (error) {
    throw new ValidationError(`GitHub returned invalid JSON: ${String(error)}`);
  }
  if (!Array.isArray(pages) || !pages.every(Array.isArray))
    throw new ValidationError("GitHub starred response was not paginated arrays.");
  const stars = pages.flat();
  if (!stars.every(isGitHubStar))
    throw new ValidationError("GitHub starred response omitted required repository fields.");
  return stars;
}

function updateRecord(old: StarRecord | undefined, star: GitHubStar, evidence: string[], now: string): StarRecord {
  const oldDecision = old?.decision === "used" ? undefined : old?.decision;
  return {
    ...old,
    fullName: star.repo.full_name,
    url: star.repo.html_url,
    description: star.repo.description ?? "",
    language: star.repo.language ?? null,
    topics: star.repo.topics ?? [],
    stars: star.repo.stargazers_count,
    archived: star.repo.archived,
    pushedAt: star.repo.pushed_at ?? null,
    starredAt: star.starred_at,
    firstSeenAt: old?.firstSeenAt ?? now,
    lastSeenAt: now,
    status: old?.decision === "used" || old?.status === "gone" ? "unreviewed" : (old?.status ?? "unreviewed"),
    decision: oldDecision,
    evidence,
    reviewedAt: old?.decision === "used" ? undefined : old?.reviewedAt,
    goneAt: undefined,
  };
}

export function mergeStars(
  previous: Ledger | null,
  user: string,
  stars: GitHubStar[],
  evidence: Map<string, string[]>,
  now = new Date().toISOString(),
): { ledger: Ledger } {
  const records = previous?.records ?? {};
  const current = new Set(stars.map((star) => star.repo.full_name.toLowerCase()));
  for (const star of stars) {
    const key = star.repo.full_name.toLowerCase();
    const old = records[key];
    records[key] = updateRecord(old, star, evidence.get(key) ?? [], now);
  }
  for (const [key, record] of Object.entries(records)) {
    if (!current.has(key) && record.status !== "gone") Object.assign(record, { status: "gone", goneAt: now });
  }
  return {
    ledger: { version: 1, githubUser: user, baselineAt: previous?.baselineAt ?? now, syncedAt: now, records },
  };
}

function eligible(record: StarRecord, now: string): boolean {
  if (record.status === "unreviewed") return true;
  return record.status === "snoozed" && Boolean(record.reviewAfter && record.reviewAfter <= now);
}

export function matchingProjects(record: StarRecord, projects: string[]): string[] {
  const text = `${record.fullName} ${record.description} ${(record.topics ?? []).join(" ")}`
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ");
  return projects.filter((project) => {
    const name = project
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, " ")
      .trim();
    return name.length >= 4 && text.includes(name);
  });
}

function reviewPriority(record: StarRecord, now: string, projects: string[]): number {
  const pushedAt = record.pushedAt ? Date.parse(record.pushedAt) : NaN;
  const recent = Number.isFinite(pushedAt) && pushedAt >= Date.parse(now) - 365 * 24 * 60 * 60 * 1000;
  return (matchingProjects(record, projects).length ? 3 : 0) + (recent ? 1 : 0) - (record.archived ? 2 : 0);
}

export function reviewQueue(
  ledger: Ledger,
  limit: number,
  now = new Date().toISOString(),
  projects: string[] = [],
): StarRecord[] {
  const records = Object.values(ledger.records).filter((record) => eligible(record, now));
  const groups = Map.groupBy(records, (record) => reviewPriority(record, now, projects));
  const result: StarRecord[] = [];
  for (const priority of [...groups.keys()].sort((a, b) => b - a)) {
    const group = groups.get(priority)!.sort((a, b) => b.starredAt.localeCompare(a.starredAt));
    let newest = 0;
    let oldest = group.length - 1;
    while (newest <= oldest && result.length < limit) {
      result.push(group[newest++]);
      if (newest <= oldest && result.length < limit) result.push(group[oldest--]);
    }
    if (result.length === limit) break;
  }
  return result;
}

export function pendingActions(ledger: Ledger): StarRecord[] {
  return Object.values(ledger.records)
    .filter(
      (record) =>
        record.status !== "gone" &&
        record.decision &&
        ACTION_DECISIONS.includes(record.decision as (typeof ACTION_DECISIONS)[number]) &&
        !record.actionCompletedAt,
    )
    .sort((a, b) => (a.reviewedAt ?? "").localeCompare(b.reviewedAt ?? ""));
}

export function completeAction(record: StarRecord, now = new Date()): void {
  if (
    !record.decision ||
    !ACTION_DECISIONS.includes(record.decision as (typeof ACTION_DECISIONS)[number]) ||
    record.status === "gone" ||
    record.actionCompletedAt
  ) {
    throw new Error(`${record.fullName} has no pending action.`);
  }
  record.actionCompletedAt = now.toISOString();
}

export function summarizeLedger(ledger: Ledger): LedgerSummary {
  const counts = Object.values(ledger.records).reduce<Record<string, number>>((out, record) => {
    out[record.status] = (out[record.status] ?? 0) + 1;
    return out;
  }, {});
  return {
    account: ledger.githubUser,
    current: Object.keys(ledger.records).length - (counts.gone ?? 0),
    review: Object.values(ledger.records).filter((record) => eligible(record, new Date().toISOString())).length,
    resolved: counts.resolved ?? 0,
    unstarCandidates: counts["unstar-candidate"] ?? 0,
    snoozed: counts.snoozed ?? 0,
    gone: counts.gone ?? 0,
    pendingActions: pendingActions(ledger).length,
    lastSync: ledger.syncedAt,
  };
}

export function formatQueue(records: StarRecord[], projects: string[] = []): string {
  if (!records.length) return "No stars are due for review.\n";
  return `${records
    .map((record, index) => {
      const matches = matchingProjects(record, projects);
      const meta = [
        record.language,
        record.archived ? "archived" : "",
        `starred ${record.starredAt.slice(0, 10)}`,
        record.pushedAt ? `Pushed ${record.pushedAt.slice(0, 10)}` : "",
        matches.length ? `Project fit: ${matches.join(", ")}` : "",
        record.topics.length ? `Topics: ${record.topics.slice(0, 5).join(", ")}` : "",
      ]
        .filter(Boolean)
        .join(" · ");
      return `${index + 1}. ${record.fullName}  ${meta}\n   ${record.description || "(no description)"}\n   ${record.url}`;
    })
    .join("\n\n")}\n`;
}

export function applyDecision(record: StarRecord, decision: Decision, note = "", project = "", now = new Date()): void {
  record.decision = decision;
  record.note = note || undefined;
  record.project = project || undefined;
  record.reviewedAt = now.toISOString();
  record.reviewAfter = undefined;
  record.actionCompletedAt = undefined;
  if (decision === "unstar") record.status = "unstar-candidate";
  else if (decision === "later") {
    record.status = "snoozed";
    now.setDate(now.getDate() + 30);
    record.reviewAfter = now.toISOString();
  } else record.status = "resolved";
}

export function parseActiveProjects(markdown: string): string[] {
  return markdown
    .split("\n")
    .filter((line) => /^\| [^|]+ \|/.test(line) && /\| (Active|Planning|Starting) \|/.test(line))
    .map((line) => line.split("|")[2]?.trim())
    .filter((name): name is string => Boolean(name));
}

function oneLine(record: StarRecord): string {
  const meta = [record.language, record.archived ? "archived" : "", `starred ${record.starredAt.slice(0, 10)}`]
    .filter(Boolean)
    .join("; ");
  const description = (record.description || "No description").replaceAll("—", "-");
  return `- [${record.fullName}](${record.url}) - ${description} (${meta})`;
}

function renderResolved(records: StarRecord[]): string[] {
  return records
    .sort((a, b) => (b.reviewedAt ?? "").localeCompare(a.reviewedAt ?? ""))
    .map((record) => {
      const project = record.project ? ` -> ${record.project}` : "";
      const note = record.note ? ` - ${record.note.trim().replaceAll("—", "-")}` : "";
      const evidence = record.evidence.length ? ` - Evidence: ${record.evidence.join(", ")}` : "";
      return `${oneLine(record)} - **${record.decision}**${project}${note}${evidence}`;
    });
}

export function renderMarkdown(ledger: Ledger, projects: string[]): string {
  const all = Object.values(ledger.records);
  const queue = reviewQueue(ledger, Number.MAX_SAFE_INTEGER, new Date().toISOString(), projects);
  const actions = pendingActions(ledger);
  const candidates = all.filter((record) => record.status === "unstar-candidate");
  const resolved = all.filter((record) => record.status === "resolved");
  const snoozed = all.filter((record) => record.status === "snoozed");
  const gone = all.filter((record) => record.status === "gone");
  return `${[
    "# GitHub Stars Review",
    "",
    `Baseline: ${ledger.baselineAt} | Last sync: ${ledger.syncedAt} | Account: ${ledger.githubUser}`,
    "",
    `Current: ${all.length - gone.length} | Review: ${queue.length} | Resolved: ${resolved.length} | Pending actions: ${actions.length} | Unstar candidates: ${candidates.length} | Gone: ${gone.length}`,
    "",
    "Run `stars review --limit 5` for the multiple-choice review. Unstarring only happens via `stars unstar`, with confirmation for every repo.",
    "",
    "## Active project context",
    "",
    ...projects.map((project) => `- ${project}`),
    "",
    "## Review queue",
    "",
    ...queue.map(oneLine),
    "",
    "## Pending actions",
    "",
    ...actions.map(
      (record) =>
        `${oneLine(record)} - **${record.decision}**${record.project ? ` -> ${record.project}` : ""}${record.note ? ` - ${record.note.trim().replaceAll("—", "-")}` : ""}`,
    ),
    "",
    "## Unstar candidates",
    "",
    ...candidates.map(
      (record) => `${oneLine(record)}${record.note ? ` - Note: ${record.note.replaceAll("—", "-")}` : ""}`,
    ),
    "",
    "## Resolved",
    "",
    ...renderResolved(resolved),
    "",
    "## Snoozed",
    "",
    ...snoozed.map((record) => `${oneLine(record)} - Review after ${record.reviewAfter?.slice(0, 10)}`),
    "",
    "## No longer starred",
    "",
    ...gone.map(oneLine),
    "",
  ]
    .join("\n")
    .trimEnd()}\n`;
}
