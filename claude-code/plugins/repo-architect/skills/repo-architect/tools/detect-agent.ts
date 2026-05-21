#!/usr/bin/env bun
/**
 * detect-agent.ts — Detect which AI coding agent is running the current session.
 *
 * Returns one of: "claude-code" | "codex-cli" | "gemini-cli" | "openclaw" | "unknown"
 *
 * Detection is based on environment variables set by each agent's runtime.
 * Designed to be both runnable (`bun run detect-agent.ts`) and importable.
 *
 * Usage:
 *   bun run ${CLAUDE_PLUGIN_ROOT}/skills/repo-architect/tools/detect-agent.ts
 *   → prints the agent ID to stdout
 *
 * Import:
 *   import { detectAgent, AgentId } from './detect-agent'
 */

export type AgentId = "claude-code" | "codex-cli" | "gemini-cli" | "openclaw" | "unknown";

export interface AgentInfo {
  id: AgentId;
  /** Human-readable name */
  name: string;
  /** Whether this agent can delegate to Codex CLI for headless execution */
  canDelegateToCodex: boolean;
  /** Whether this agent can execute file moves directly */
  canExecuteDirectly: boolean;
}

const AGENTS: Record<AgentId, Omit<AgentInfo, "id">> = {
  "claude-code": {
    name: "Claude Code",
    canDelegateToCodex: true,
    canExecuteDirectly: true,
  },
  "codex-cli": {
    name: "Codex CLI",
    canDelegateToCodex: false, // Would be circular
    canExecuteDirectly: true,
  },
  "gemini-cli": {
    name: "Gemini CLI",
    canDelegateToCodex: true,
    canExecuteDirectly: true,
  },
  openclaw: {
    name: "OpenClaw",
    canDelegateToCodex: true,
    canExecuteDirectly: true,
  },
  unknown: {
    name: "Unknown Agent",
    canDelegateToCodex: false,
    canExecuteDirectly: true,
  },
};

/**
 * Detect the current AI coding agent by checking environment variables.
 *
 * Priority order matters: more specific markers are checked first.
 * Claude Code sets CLAUDECODE=1. Codex sets CODEX_SANDBOX or CODEX_HOME.
 * Gemini sets GEMINI_CLI_* vars. OpenClaw sets OPENCLAW_HOME.
 */
export function detectAgent(): AgentId {
  const env = process.env;

  // Claude Code — most specific single marker
  if (env.CLAUDECODE === "1") return "claude-code";

  // Codex CLI — sandbox or home directory marker
  if (env.CODEX_SANDBOX || env.CODEX_HOME) return "codex-cli";

  // Gemini CLI — no single marker; check IDE server port or sandbox flag
  if (env.GEMINI_CLI_IDE_SERVER_PORT || env.GEMINI_SANDBOX || env.GEMINI_CLI_SYSTEM_SETTINGS_PATH) {
    return "gemini-cli";
  }

  // OpenClaw — home directory or service marker
  if (env.OPENCLAW_HOME || env.OPENCLAW_SERVICE_MARKER) return "openclaw";

  return "unknown";
}

/**
 * Get full agent info including capabilities.
 */
export function getAgentInfo(): AgentInfo {
  const id = detectAgent();
  return { id, ...AGENTS[id] };
}

// CLI mode: print agent ID when run directly
if (import.meta.main) {
  const flag = process.argv[2];
  if (flag === "--json") {
    console.log(JSON.stringify(getAgentInfo(), null, 2));
  } else if (flag === "--verbose") {
    const info = getAgentInfo();
    console.log(`Agent: ${info.name} (${info.id})`);
    console.log(`Can delegate to Codex: ${info.canDelegateToCodex}`);
    console.log(`Can execute directly: ${info.canExecuteDirectly}`);
  } else {
    console.log(detectAgent());
  }
}
