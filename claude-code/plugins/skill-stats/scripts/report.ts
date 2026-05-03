#!/usr/bin/env bun
/**
 * skill-stats report generator
 *
 * Reads plugin-owned telemetry written by hooks/log-skill.ts and produces
 * either a human-readable text report or machine-readable JSON.
 *
 * Telemetry path resolution mirrors log-skill.ts:
 *   1. $CLAUDE_PLUGIN_DATA/events.jsonl
 *   2. $HOME/.claude/plugins/data/skill-stats/events.jsonl
 *
 * Skill discovery scans the standard Claude Code skill locations:
 *   - $HOME/.claude/skills/
 *   - $HOME/.claude/plugins/marketplaces/
 *
 * Flags:
 *   --json              machine-readable JSON
 *   --days=N            restrict telemetry window to last N days
 *   --stale=N           custom stale threshold in days (default 30)
 *   --author=NAME       filter to skills whose SKILL.md frontmatter `authors:`
 *                       (or `author:`) contains NAME
 */

import { readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";

const HOME = homedir();
const SKILLS_DIRS = [`${HOME}/.claude/skills`, `${HOME}/.claude/plugins/marketplaces`];

function resolveTelemetryPath(): string {
  const data = process.env.CLAUDE_PLUGIN_DATA;
  if (data && data.length > 0) return `${data}/events.jsonl`;
  return `${HOME}/.claude/plugins/data/skill-stats/events.jsonl`;
}

const TELEMETRY = resolveTelemetryPath();
const NOW = Date.now();
const DAY_MS = 86_400_000;

type Installed = {
  name: string;
  emoji: string;
  path: string;
  source: "user" | "plugin";
  marketplace?: string;
  mtime: number;
  sizeBytes: number;
  authors: string[];
};

type Usage = {
  count: number;
  firstUsed: number;
  lastUsed: number;
  sessions: Set<string>;
};

const args = process.argv.slice(2);
const flags = {
  json: args.includes("--json"),
  windowDays: parseInt(args.find((a) => a.startsWith("--days="))?.split("=")[1] ?? "0", 10),
  staleDays: parseInt(args.find((a) => a.startsWith("--stale="))?.split("=")[1] ?? "30", 10),
  author: args.find((a) => a.startsWith("--author="))?.split("=")[1] ?? null,
};

function dirSize(dir: string): number {
  let total = 0;
  function walk(d: string) {
    let entries: ReturnType<typeof readdirSync>;
    try {
      entries = readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const ent of entries) {
      const full = join(d, ent.name);
      try {
        const s = statSync(full);
        if (s.isFile()) total += s.size;
        else if (s.isDirectory()) walk(full);
      } catch {}
    }
  }
  walk(dir);
  return total;
}

function parseAuthors(fm: string): string[] {
  const lines = fm.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^author[s]?\s*:\s*(.*)$/);
    if (!m) continue;
    const rest = m[1].trim();
    if (rest.startsWith("[") && rest.endsWith("]")) {
      return rest
        .slice(1, -1)
        .split(",")
        .map((s) => s.trim().replace(/^["']|["']$/g, ""))
        .filter(Boolean);
    }
    if (rest) return [rest.replace(/^["']|["']$/g, "")];
    const out: string[] = [];
    for (let j = i + 1; j < lines.length; j++) {
      const item = lines[j].match(/^\s+-\s+(.*)$/);
      if (!item) break;
      out.push(item[1].trim().replace(/^["']|["']$/g, ""));
    }
    return out;
  }
  return [];
}

function parseFrontmatter(file: string): { name: string | null; emoji: string; authors: string[] } {
  try {
    const text = readFileSync(file, "utf8");
    const m = text.match(/^---\n([\s\S]*?)\n---/);
    if (!m) return { name: null, emoji: "", authors: [] };
    const lines = m[1].split("\n");
    const nameLine = lines.find((l) => l.startsWith("name:"));
    const emojiLine = lines.find((l) => l.startsWith("emoji:"));
    const name = nameLine
      ? nameLine
          .replace(/^name:\s*/, "")
          .trim()
          .replace(/^["']|["']$/g, "")
      : null;
    const emoji = emojiLine
      ? emojiLine
          .replace(/^emoji:\s*/, "")
          .trim()
          .replace(/^["']|["']$/g, "")
      : "";
    return { name, emoji, authors: parseAuthors(m[1]) };
  } catch {
    return { name: null, emoji: "", authors: [] };
  }
}

function findSkillFiles(root: string, maxDepth: number): string[] {
  const found: string[] = [];
  const seen = new Set<string>();

  function walk(dir: string, depth: number) {
    if (depth > maxDepth) return;
    let entries: ReturnType<typeof readdirSync>;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const ent of entries) {
      const full = join(dir, ent.name);
      let real: string;
      try {
        real = realpathSync(full);
      } catch {
        continue;
      }
      if (seen.has(real)) continue;
      seen.add(real);
      if (ent.name === "SKILL.md" && ent.isFile()) {
        found.push(full);
        continue;
      }
      let isDir = ent.isDirectory();
      if (ent.isSymbolicLink()) {
        try {
          isDir = statSync(full).isDirectory();
        } catch {
          isDir = false;
        }
      }
      if (isDir && !ent.name.startsWith(".") && ent.name !== "node_modules") {
        walk(full, depth + 1);
      }
    }
  }

  walk(root, 0);
  return found;
}

function discoverInstalled(): Installed[] {
  const out: Installed[] = [];
  const seenPaths = new Set<string>();

  for (const file of findSkillFiles(SKILLS_DIRS[0], 2)) {
    const fm = parseFrontmatter(file);
    const name = fm.name ?? basename(dirname(file));
    const dir = dirname(file);
    const realDir = realpathSync(dir);
    if (seenPaths.has(realDir)) continue;
    seenPaths.add(realDir);
    const s = statSync(dir);
    out.push({
      name,
      emoji: fm.emoji,
      path: dir,
      source: "user",
      mtime: s.mtimeMs,
      sizeBytes: dirSize(realDir),
      authors: fm.authors,
    });
  }

  for (const file of findSkillFiles(SKILLS_DIRS[1], 5)) {
    if (file.includes("/temp_")) continue;
    const fm = parseFrontmatter(file);
    const name = fm.name ?? basename(dirname(file));
    const rel = file.replace(SKILLS_DIRS[1] + "/", "");
    const marketplace = rel.split("/")[0];
    const dir = dirname(file);
    const realDir = realpathSync(dir);
    if (seenPaths.has(realDir)) continue;
    seenPaths.add(realDir);
    const s = statSync(dir);
    out.push({
      name,
      emoji: fm.emoji,
      path: dir,
      source: "plugin",
      marketplace,
      mtime: s.mtimeMs,
      sizeBytes: dirSize(realDir),
      authors: fm.authors,
    });
  }

  return out;
}

function parseTelemetry(): Map<string, Usage> {
  const usage = new Map<string, Usage>();
  let raw: string;
  try {
    raw = readFileSync(TELEMETRY, "utf8");
  } catch {
    return usage;
  }

  const cutoff = flags.windowDays > 0 ? NOW - flags.windowDays * DAY_MS : 0;

  for (const line of raw.split("\n")) {
    if (!line.trim()) continue;
    let evt: { event?: string; skill?: string; session_id?: string; timestamp?: string };
    try {
      evt = JSON.parse(line);
    } catch {
      continue;
    }
    if (evt.event !== "skill_invocation") continue;
    if (!evt.skill) continue;
    if (evt.session_id === "smoke" || evt.session_id === "test") continue;

    const ts = evt.timestamp ? new Date(evt.timestamp).getTime() : NaN;
    if (!Number.isFinite(ts)) continue;
    if (cutoff && ts < cutoff) continue;

    const name = evt.skill;
    const u = usage.get(name) ?? ({ count: 0, firstUsed: ts, lastUsed: ts, sessions: new Set<string>() } as Usage);
    u.count += 1;
    u.firstUsed = Math.min(u.firstUsed, ts);
    u.lastUsed = Math.max(u.lastUsed, ts);
    if (evt.session_id) u.sessions.add(evt.session_id);
    usage.set(name, u);
  }
  return usage;
}

function normalize(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function matchUsage(installed: Installed[], usage: Map<string, Usage>) {
  const usageByNorm = new Map<string, { name: string; data: Usage }>();
  for (const [name, data] of usage) usageByNorm.set(normalize(name), { name, data });

  const seenNames = new Set<string>();
  const matched: Array<{ skill: Installed; usage: Usage; invokedAs: string; duplicates: number }> = [];
  const dormant: Installed[] = [];
  const duplicateCount = new Map<string, number>();

  for (const skill of installed) {
    const norm = normalize(skill.name);
    if (seenNames.has(norm)) {
      duplicateCount.set(norm, (duplicateCount.get(norm) ?? 1) + 1);
      continue;
    }
    seenNames.add(norm);
    const hit = usageByNorm.get(norm);
    if (hit) {
      matched.push({ skill, usage: hit.data, invokedAs: hit.name, duplicates: 0 });
    } else {
      dormant.push(skill);
    }
  }

  for (const m of matched) {
    m.duplicates = (duplicateCount.get(normalize(m.skill.name)) ?? 1) - 1;
  }

  const installedNorms = new Set([...seenNames]);
  const phantom: Array<{ name: string; data: Usage }> = [];
  for (const [name, data] of usage) {
    if (!installedNorms.has(normalize(name))) phantom.push({ name, data });
  }

  return { matched, dormant, phantom };
}

function fmtAge(ms: number): string {
  const days = Math.floor((NOW - ms) / DAY_MS);
  if (days === 0) return "today";
  if (days === 1) return "1 day ago";
  return `${days} days ago`;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}K`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}M`;
}

function main() {
  const installed = discoverInstalled();
  const usage = parseTelemetry();
  const { matched, dormant, phantom } = matchUsage(installed, usage);

  if (flags.author) {
    const wanted = flags.author.toLowerCase();
    const mine = installed.filter((s) => s.authors.some((a) => a.toLowerCase() === wanted));
    const usageByNorm = new Map<string, Usage>();
    for (const [n, u] of usage) usageByNorm.set(normalize(n), u);

    if (flags.json) {
      process.stdout.write(
        JSON.stringify(
          {
            generated_at: new Date(NOW).toISOString(),
            telemetry_path: TELEMETRY,
            author: flags.author,
            count: mine.length,
            skills: mine.map((s) => {
              const u = usageByNorm.get(normalize(s.name));
              return {
                name: s.name,
                emoji: s.emoji,
                source: s.source,
                marketplace: s.marketplace,
                size_bytes: s.sizeBytes,
                authors: s.authors,
                count: u?.count ?? 0,
                last_used: u ? new Date(u.lastUsed).toISOString() : null,
                sessions: u?.sessions.size ?? 0,
              };
            }),
          },
          null,
          2,
        ) + "\n",
      );
      return;
    }

    process.stdout.write("╔══════════════════════════════════════════════════════════════╗\n");
    process.stdout.write(`║  Skills authored by ${flags.author.padEnd(41)} ║\n`);
    process.stdout.write("╚══════════════════════════════════════════════════════════════╝\n");
    process.stdout.write(`Total authored:  ${mine.length}\n`);
    const totalSize = mine.reduce((a, b) => a + b.sizeBytes, 0);
    process.stdout.write(`Total disk:      ${fmtSize(totalSize)}\n`);
    const used = mine.filter((s) => usageByNorm.has(normalize(s.name)));
    process.stdout.write(`Used:            ${used.length}    Dormant: ${mine.length - used.length}\n\n`);

    const rows = mine
      .map((s) => ({ s, u: usageByNorm.get(normalize(s.name)) }))
      .sort((a, b) => (b.u?.count ?? 0) - (a.u?.count ?? 0));

    process.stdout.write(
      `  ${"name".padEnd(32)}  ${"count".padStart(5)}  ${"sessions".padStart(8)}  ${"last used".padEnd(14)}  size\n`,
    );
    process.stdout.write("  " + "─".repeat(76) + "\n");
    for (const { s, u } of rows) {
      const emoji = s.emoji ? `${s.emoji} ` : "  ";
      const last = u ? fmtAge(u.lastUsed) : "—";
      process.stdout.write(
        `  ${emoji}${s.name.padEnd(30)}  ${(u?.count ?? 0).toString().padStart(5)}  ${(u?.sessions.size ?? 0).toString().padStart(8)}  ${last.padEnd(14)}  ${fmtSize(s.sizeBytes)}\n`,
      );
    }
    return;
  }

  if (flags.json) {
    process.stdout.write(
      JSON.stringify(
        {
          generated_at: new Date(NOW).toISOString(),
          telemetry_path: TELEMETRY,
          window_days: flags.windowDays || "all",
          totals: {
            installed: installed.length,
            matched: matched.length,
            dormant: dormant.length,
            phantom: phantom.length,
            total_invocations: [...usage.values()].reduce((a, b) => a + b.count, 0),
          },
          matched: matched.map((m) => ({
            name: m.skill.name,
            emoji: m.skill.emoji,
            invoked_as: m.invokedAs,
            count: m.usage.count,
            first_used: new Date(m.usage.firstUsed).toISOString(),
            last_used: new Date(m.usage.lastUsed).toISOString(),
            sessions: m.usage.sessions.size,
            source: m.skill.source,
            size_bytes: m.skill.sizeBytes,
            authors: m.skill.authors,
          })),
          dormant: dormant.map((s) => ({
            name: s.name,
            emoji: s.emoji,
            source: s.source,
            marketplace: s.marketplace,
            mtime: new Date(s.mtime).toISOString(),
            size_bytes: s.sizeBytes,
            authors: s.authors,
          })),
          phantom: phantom.map((p) => ({
            name: p.name,
            count: p.data.count,
            last_used: new Date(p.data.lastUsed).toISOString(),
          })),
        },
        null,
        2,
      ) + "\n",
    );
    return;
  }

  const totalInvocations = [...usage.values()].reduce((a, b) => a + b.count, 0);

  process.stdout.write("╔══════════════════════════════════════════════════════════════╗\n");
  process.stdout.write("║  Claude Code Skill Telemetry — Usage & Dormancy Report       ║\n");
  process.stdout.write("╚══════════════════════════════════════════════════════════════╝\n");
  process.stdout.write(`Generated:        ${new Date(NOW).toISOString()}\n`);
  process.stdout.write(`Telemetry file:   ${TELEMETRY}\n`);
  process.stdout.write(
    `Window:           ${flags.windowDays > 0 ? `last ${flags.windowDays} days` : "all telemetry"}\n`,
  );

  if (totalInvocations > 0) {
    const earliest = Math.min(...[...usage.values()].map((u) => u.firstUsed));
    const latest = Math.max(...[...usage.values()].map((u) => u.lastUsed));
    process.stdout.write(
      `Telemetry span:   ${new Date(earliest).toISOString().slice(0, 10)} → ${new Date(latest).toISOString().slice(0, 10)}\n`,
    );
  } else {
    process.stdout.write(`Telemetry span:   (no events recorded yet — hook will populate as skills are invoked)\n`);
  }

  process.stdout.write(
    `Installed skills: ${installed.length} (${installed.filter((s) => s.source === "user").length} user, ${installed.filter((s) => s.source === "plugin").length} plugin)\n`,
  );
  process.stdout.write(`Total invocations: ${totalInvocations}\n`);
  process.stdout.write(`Distinct used:    ${matched.length}\n`);
  process.stdout.write(`Dormant:          ${dormant.length}  (installed, never invoked)\n`);
  process.stdout.write(`Phantom:          ${phantom.length}  (invoked but no SKILL.md found)\n\n`);

  const emojiSlot = (e: string) => (e ? `${e} ` : "  ");

  process.stdout.write("─── TOP 20 MOST USED ──────────────────────────────────────────\n");
  const sortedMatched = [...matched].sort((a, b) => b.usage.count - a.usage.count);
  if (sortedMatched.length === 0) process.stdout.write("  (none yet)\n");
  for (const m of sortedMatched.slice(0, 20)) {
    const pct = totalInvocations > 0 ? ((m.usage.count / totalInvocations) * 100).toFixed(1) : "0.0";
    process.stdout.write(
      `  ${m.usage.count.toString().padStart(4)}  ${pct.padStart(5)}%  ${emojiSlot(m.skill.emoji)} ${m.skill.name.padEnd(26)}  last: ${fmtAge(m.usage.lastUsed)}\n`,
    );
  }
  process.stdout.write("\n");

  process.stdout.write("─── RECENTLY ACTIVE (last 7 days) ─────────────────────────────\n");
  const recent = sortedMatched.filter((m) => NOW - m.usage.lastUsed < 7 * DAY_MS);
  if (recent.length === 0) process.stdout.write("  (none)\n");
  for (const m of recent) {
    process.stdout.write(
      `  ${emojiSlot(m.skill.emoji)} ${m.skill.name.padEnd(30)}  ${m.usage.count}× over ${m.usage.sessions.size} sessions\n`,
    );
  }
  process.stdout.write("\n");

  process.stdout.write(`─── STALE (used but not in last ${flags.staleDays} days) ──────────────────\n`);
  const stale = sortedMatched.filter((m) => NOW - m.usage.lastUsed > flags.staleDays * DAY_MS);
  if (stale.length === 0) process.stdout.write("  (none)\n");
  for (const m of stale) {
    process.stdout.write(
      `  ${emojiSlot(m.skill.emoji)} ${m.skill.name.padEnd(30)}  ${m.usage.count}× — last ${fmtAge(m.usage.lastUsed)}\n`,
    );
  }
  process.stdout.write("\n");

  process.stdout.write("─── DORMANT (installed, never invoked) ────────────────────────\n");
  const sortedDormant = [...dormant].sort((a, b) => b.sizeBytes - a.sizeBytes);
  process.stdout.write(
    `  Total: ${dormant.length} skills, ${fmtSize(dormant.reduce((a, b) => a + b.sizeBytes, 0))} on disk\n\n`,
  );
  process.stdout.write(
    `  ${"name".padEnd(32)}  ${"source".padEnd(8)}  ${"size".padStart(8)}  ${"last touched".padEnd(14)}\n`,
  );
  process.stdout.write("  " + "─".repeat(72) + "\n");
  for (const s of sortedDormant) {
    const src = s.source === "plugin" ? `plugin:${s.marketplace?.slice(0, 12) ?? ""}` : "user";
    process.stdout.write(
      `  ${emojiSlot(s.emoji)} ${s.name.padEnd(30)}  ${src.padEnd(8)}  ${fmtSize(s.sizeBytes).padStart(8)}  ${fmtAge(s.mtime)}\n`,
    );
  }
  process.stdout.write("\n");

  if (phantom.length > 0) {
    process.stdout.write("─── PHANTOM (invoked but no SKILL.md found) ───────────────────\n");
    process.stdout.write("  These names appear in telemetry but don't match any installed skill.\n");
    process.stdout.write("  Likely plugin-namespaced skills or uninstalled skills:\n\n");
    const sortedPhantom = [...phantom].sort((a, b) => b.data.count - a.data.count);
    for (const p of sortedPhantom) {
      process.stdout.write(
        `  ${p.data.count.toString().padStart(4)}  ${p.name.padEnd(32)}  last: ${fmtAge(p.data.lastUsed)}\n`,
      );
    }
    process.stdout.write("\n");
  }

  process.stdout.write("─── PRUNING CANDIDATES ─────────────────────────────────────────\n");
  const pruneByBytes = sortedDormant.slice(0, 5).reduce((a, b) => a + b.sizeBytes, 0);
  process.stdout.write(`  Top 5 dormant by disk size = ${fmtSize(pruneByBytes)}\n`);
  process.stdout.write(`  All dormant disk total     = ${fmtSize(dormant.reduce((a, b) => a + b.sizeBytes, 0))}\n`);
  process.stdout.write(
    `  Reminder: dormant != deletable — telemetry only goes back as far as the hook has been collecting.\n\n`,
  );
  process.stdout.write("Flags:  --json  --days=N  --stale=N  --author=NAME\n");
}

main();
