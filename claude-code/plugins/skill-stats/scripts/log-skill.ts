#!/usr/bin/env bun
/**
 * skill-stats log-skill hook
 *
 * Fires on PreToolUse with matcher "Skill". Reads the hook input JSON from
 * stdin, extracts the skill name from `tool_input.skill`, and appends a
 * single JSONL event to the plugin's telemetry file.
 *
 * Storage path resolution (in priority order):
 *   1. $CLAUDE_PLUGIN_DATA/events.jsonl          (canonical, plugin-scoped)
 *   2. $HOME/.claude/plugins/data/skill-stats/events.jsonl  (fallback)
 *
 * The hook NEVER blocks skill execution — any error path silently exits 0.
 * The report generator (scripts/report.ts) reads from the same path.
 */

import { appendFileSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname } from "node:path";

interface HookInput {
  tool_name?: string;
  tool_input?: { skill?: string };
  session_id?: string;
  hook_event_name?: string;
}

function resolveTelemetryPath(): string {
  const data = process.env.CLAUDE_PLUGIN_DATA;
  if (data && data.length > 0) return `${data}/events.jsonl`;
  return `${homedir()}/.claude/plugins/data/skill-stats/events.jsonl`;
}

async function readStdin(timeoutMs = 1000): Promise<string> {
  return new Promise((resolve) => {
    let data = "";
    const timer = setTimeout(() => resolve(data), timeoutMs);
    process.stdin.on("data", (chunk) => {
      data += chunk.toString();
    });
    process.stdin.on("end", () => {
      clearTimeout(timer);
      resolve(data);
    });
    process.stdin.on("error", () => {
      clearTimeout(timer);
      resolve("");
    });
  });
}

async function main() {
  try {
    const raw = await readStdin();
    if (!raw) process.exit(0);

    const input: HookInput = JSON.parse(raw);
    const skill = (input.tool_input?.skill ?? "").trim();
    if (!skill) process.exit(0);

    const sessionId = input.session_id ?? "no-session";
    if (sessionId === "smoke" || sessionId === "test") process.exit(0);

    const event = {
      event: "skill_invocation",
      skill,
      session_id: sessionId,
      timestamp: new Date().toISOString(),
    };

    const path = resolveTelemetryPath();
    mkdirSync(dirname(path), { recursive: true });
    appendFileSync(path, JSON.stringify(event) + "\n");
    process.exit(0);
  } catch {
    process.exit(0);
  }
}

main();
