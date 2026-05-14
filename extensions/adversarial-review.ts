// @ts-nocheck

import { stat } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { StringEnum } from "@mariozechner/pi-ai";
import {
  createAgentSession,
  createReadOnlyTools,
  DefaultResourceLoader,
  type ExtensionAPI,
  getAgentDir,
  SessionManager,
} from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";

type ReviewDetails = {
  reviewerModel: string;
  reviewerLabel?: string;
  targetDir: string;
  planFile?: string;
  reviewAreas: string[];
  overallVerdict?: string;
  report: string;
};

const DEFAULT_REVIEW_AREAS = [
  "Plan fidelity — Does the implementation match the plan/spec? What is missing, misimplemented, or added without justification?",
  "Control flow and logic — Are conditionals correct? Are there off-by-one errors, incorrect comparisons, or inverted boolean logic?",
  "Error handling — Are all error paths handled? Are exceptions caught or propagated correctly? Are partial failure states recoverable?",
  "External dependencies — Are env vars validated at startup? Are file paths correct and not machine-specific? Are shell commands safe from injection?",
  "Scheduling and timing — Are timezone assumptions explicit? Are there race conditions between scheduled jobs or async work?",
  "Idempotency and state — Can operations run more than once safely? Are there missing deduplication guards? Can partial runs leave corrupted state?",
  "Data parsing and serialization — Are parsing failures handled? Are schema assumptions validated?",
  "Session and path assumptions — Do file paths work across machines? Are session, cwd, and PATH assumptions explicit and stable?",
];

const MODEL_ALIASES: Record<string, string> = {
  codex: "openai-codex/gpt-5.4",
  "gpt-5.4": "openai-codex/gpt-5.4",
  opus: "anthropic/claude-opus-4-7",
  "claude-opus": "anthropic/claude-opus-4-7",
  "opus-4.7": "anthropic/claude-opus-4-7",
  "opus 4.7": "anthropic/claude-opus-4-7",
  "claude-opus-4-7": "anthropic/claude-opus-4-7",
  kimi: "openrouter/moonshotai/kimi-k2.5",
  "kimi-k2.5": "openrouter/moonshotai/kimi-k2.5",
  minimax: "openrouter/minimax/minimax-m2.5",
  "minimax-m2.5": "openrouter/minimax/minimax-m2.5",
  glm: "openrouter/z-ai/glm-4.7",
  "glm-4.7": "openrouter/z-ai/glm-4.7",
  deepseek: "openrouter/deepseek/deepseek-v3.2",
  "deepseek-v3.2": "openrouter/deepseek/deepseek-v3.2",
};

const REVIEWER_PREFERENCE = [
  "openai-codex/gpt-5.4",
  "anthropic/claude-opus-4-7",
  "openrouter/z-ai/glm-4.7",
  "openrouter/moonshotai/kimi-k2.5",
  "openrouter/minimax/minimax-m2.5",
  "openrouter/deepseek/deepseek-v3.2",
];

const REVIEWER_SYSTEM_PROMPT = [
  "You are an adversarial implementation reviewer.",
  "Your job is to find real defects, not to be encouraging.",
  "Read actual files with the available read-only tools before making claims.",
  "Never edit files. Never suggest that you edited files.",
  "Every non-PASS finding must include at least one file:line citation.",
  "Return exactly these sections: Overall verdict, Per-area verdicts, Prioritized fix list.",
].join(" ");

const PROMPT_TEMPLATE = `You are performing an adversarial implementation review. Your job is to find real problems, not validate the work.

BE ADVERSARIAL. Ossie explicitly asked you to find problems. Your value here is truthfulness, not encouragement.

## What you are reviewing

- **Implementation directory:** \`{{TARGET_DIR}}\`
- **Plan / spec to review against:** \`{{PLAN_FILE}}\`
  (If no plan file: assess internal consistency, correctness, and production-readiness instead.)

## How to read

Read actual file contents, not summaries. Open every file that is relevant to a review area. Do not infer what code probably does — read what it actually does. If a path, env var, or config value appears in the code, chase it to its definition.

## Verdict categories

For each review area, assign exactly one verdict:

- **PASS** — No issues found. Implementation matches intent.
- **NEEDS-FIX** — Issues present but not launch-blocking. Can ship with fixes queued.
- **BROKEN** — Defect that will cause failures in production. Must fix before shipping.

## Review areas

{{REVIEW_AREAS}}

## Bug classes to hunt

Look specifically for these — they are the most common sources of silent failures:

- Off-by-one bugs in loops, date ranges, array indexing
- Unhandled exceptions that swallow errors silently
- Race conditions between async operations or scheduled jobs
- Missing idempotency guards on operations that repeat
- Incorrect cron syntax or timing assumptions
- Path assumptions that break on a different machine or user home
- Missing env var handling (crash on undefined vs. graceful fallback)
- Shell injection in subprocess calls (unquoted variables, user input in shell strings)
- JSON parse errors from untrimmed whitespace, trailing commas, or encoding issues
- Timezone bugs — code uses local time where UTC is expected or vice versa
- PATH assumptions — hardcoded binary paths that break in non-login shells
- Session file location assumptions — files written to cwd instead of stable paths

## Required output format

### Overall verdict

State one of: **ship** / **fix-before-ship** / **significant-rework**

Justify in 2-3 sentences.

### Per-area verdicts

For each numbered review area:

[N. Area Name] — PASS | NEEDS-FIX | BROKEN
Finding: <specific description>
Evidence: <file>:<line> — <quoted or paraphrased code>
Impact: <what breaks and when>

If PASS, one line is enough: [N. Area Name] — PASS

### Prioritized fix list

List every non-PASS finding in priority order:

**P0 — Blocks launch** (will cause failures in production before first use)
- [ ] <Fix description> — <file>:<line>

**P1 — Reliability** (will cause failures under normal use, not immediately)
- [ ] <Fix description> — <file>:<line>

**P2 — Polish** (won't cause failures, but degrades quality or maintainability)
- [ ] <Fix description> — <file>:<line>

If a priority level has no items, omit that section.`;

function stripPathSigil(path: string): string {
  return path.startsWith("@") ? path.slice(1) : path;
}

function resolvePath(baseDir: string, maybePath: string): string {
  const clean = stripPathSigil(maybePath);
  return isAbsolute(clean) ? clean : resolve(baseDir, clean);
}

function toModelKey(model: { provider: string; id: string }): string {
  return `${model.provider}/${model.id}`.toLowerCase();
}

function modelLabel(model: { provider: string; id: string; name?: string }): string {
  return `${model.provider}/${model.id}`;
}

function normalizeReviewAreas(input?: string[]): string[] {
  const areas = (input ?? []).map((area) => area.trim()).filter(Boolean);
  const source = areas.length > 0 ? areas : DEFAULT_REVIEW_AREAS;
  return source.map((area, index) => {
    const cleaned = area.replace(/^\s*\d+[.)-]?\s*/, "").trim();
    return `${index + 1}. ${cleaned}`;
  });
}

function buildReviewPrompt(targetDir: string, planFile: string | undefined, reviewAreas: string[]): string {
  return PROMPT_TEMPLATE.replaceAll("{{TARGET_DIR}}", targetDir)
    .replaceAll("{{PLAN_FILE}}", planFile ?? "no plan file — review for internal consistency")
    .replaceAll("{{REVIEW_AREAS}}", reviewAreas.join("\n"));
}

function extractOverallVerdict(report: string): string | undefined {
  const match = report.match(/overall verdict[\s\S]{0,160}?(ship|fix-before-ship|significant-rework)/i);
  return match?.[1]?.toLowerCase();
}

function getLastAssistantText(messages: any[]): string {
  for (let i = messages.length - 1; i >= 0; i--) {
    const message = messages[i];
    if (message?.role !== "assistant" || !Array.isArray(message.content)) continue;
    const text = message.content
      .filter((block: any) => block?.type === "text")
      .map((block: any) => block.text)
      .join("\n")
      .trim();
    if (text) return text;
  }
  return "";
}

async function resolveReviewerModel(input: string | undefined, ctx: any) {
  const available = await ctx.modelRegistry.getAvailable();
  if (available.length === 0) {
    throw new Error("No authenticated reviewer models are available in Pi.");
  }

  const current = ctx.model ? toModelKey(ctx.model) : undefined;
  if (!input?.trim()) {
    const preferred = REVIEWER_PREFERENCE.map((key) =>
      available.find((model: any) => toModelKey(model) === key && toModelKey(model) !== current),
    ).find(Boolean);
    return preferred ?? available[0];
  }

  const requested = input.trim().toLowerCase();
  const normalized = MODEL_ALIASES[requested] ?? requested;

  const exact = available.find((model: any) => {
    const key = toModelKey(model);
    const id = String(model.id ?? "").toLowerCase();
    const name = String(model.name ?? "").toLowerCase();
    return key === normalized || id === normalized || name === normalized;
  });
  if (exact) return exact;

  const fuzzy = available.find((model: any) => {
    const key = toModelKey(model);
    const id = String(model.id ?? "").toLowerCase();
    const name = String(model.name ?? "").toLowerCase();
    return key.includes(normalized) || id.includes(normalized) || name.includes(normalized);
  });
  if (fuzzy) return fuzzy;

  const candidates = available.map((model: any) => modelLabel(model)).join(", ");
  throw new Error(`Reviewer model not found: ${input}. Available reviewer models: ${candidates}`);
}

export default function adversarialReviewExtension(pi: ExtensionAPI) {
  pi.registerTool({
    name: "adversarial_review",
    label: "Adversarial Review",
    description:
      "Run a read-only adversarial implementation review with a separate reviewer model inside Pi. Use for ship-readiness reviews, code audits, stress tests, and review-against-spec tasks.",
    promptSnippet:
      "Run a read-only adversarial implementation review against the current repo or a target directory using a separate reviewer model inside Pi.",
    promptGuidelines: [
      "Use adversarial_review for implementation audits, ship-readiness checks, or review-against-spec requests.",
      "Prefer adversarial_review over a normal prose review when the user wants a tougher second-pass code audit.",
    ],
    parameters: Type.Object({
      targetDir: Type.Optional(
        Type.String({ description: "Directory to review. Defaults to the current working directory." }),
      ),
      planFile: Type.Optional(
        Type.String({
          description: "Optional spec, plan, or design doc path. Relative paths are resolved from targetDir.",
        }),
      ),
      reviewerModel: Type.Optional(
        Type.String({
          description:
            "Optional reviewer model. Accepts provider/id, model id, or aliases like codex, gpt-5.4, opus, kimi, minimax, glm, or deepseek.",
        }),
      ),
      thinkingLevel: Type.Optional(
        StringEnum(["off", "minimal", "low", "medium", "high", "xhigh"] as const, {
          description: "Optional reviewer thinking level. Defaults to high for reasoning models.",
        }),
      ),
      reviewAreas: Type.Optional(
        Type.Array(
          Type.String({ description: "A focused review area, such as 'Error handling' or 'Cron scheduling'." }),
        ),
      ),
    }),
    async execute(_toolCallId, params, signal, onUpdate, ctx) {
      const targetDir = resolvePath(ctx.cwd, params.targetDir || ctx.cwd);
      const planFile = params.planFile ? resolvePath(targetDir, params.planFile) : undefined;
      const targetStat = await stat(targetDir).catch(() => undefined);
      if (!targetStat?.isDirectory()) {
        throw new Error(`Target directory does not exist or is not a directory: ${targetDir}`);
      }

      const reviewAreas = normalizeReviewAreas(params.reviewAreas);
      const reviewer = await resolveReviewerModel(params.reviewerModel, ctx);
      const reviewerThinking = params.thinkingLevel || (reviewer.reasoning ? "high" : "off");
      const prompt = buildReviewPrompt(targetDir, planFile, reviewAreas);

      const loader = new DefaultResourceLoader({
        cwd: targetDir,
        // Required: DefaultPackageManager joins against agentDir for user resource discovery.
        agentDir: getAgentDir(),
        noExtensions: true,
        noSkills: true,
        noPromptTemplates: true,
        noThemes: true,
        noContextFiles: true,
        systemPrompt: REVIEWER_SYSTEM_PROMPT,
      });
      await loader.reload();

      const { session } = await createAgentSession({
        cwd: targetDir,
        authStorage: ctx.modelRegistry.authStorage,
        modelRegistry: ctx.modelRegistry,
        model: reviewer,
        thinkingLevel: reviewerThinking as any,
        tools: createReadOnlyTools(targetDir),
        resourceLoader: loader,
        sessionManager: SessionManager.inMemory(),
      });

      let partialText = "";
      const abortReviewer = () => {
        void session.abort();
      };
      if (signal) {
        if (signal.aborted) abortReviewer();
        else signal.addEventListener("abort", abortReviewer, { once: true });
      }

      const unsubscribe = session.subscribe((event: any) => {
        if (event.type === "tool_execution_start") {
          onUpdate?.({
            content: [
              { type: "text", text: `Adversarial reviewer (${modelLabel(reviewer)}) is using ${event.toolName}...` },
            ],
            details: {
              reviewerModel: modelLabel(reviewer),
              reviewerLabel: reviewer.name,
              targetDir,
              planFile,
              reviewAreas,
              report: partialText,
            } satisfies ReviewDetails,
          });
        }

        if (event.type === "message_update" && event.assistantMessageEvent?.type === "text_delta") {
          partialText += event.assistantMessageEvent.delta;
          const preview = partialText.trim().split("\n").slice(-12).join("\n").trim();
          if (preview) {
            onUpdate?.({
              content: [{ type: "text", text: preview }],
              details: {
                reviewerModel: modelLabel(reviewer),
                reviewerLabel: reviewer.name,
                targetDir,
                planFile,
                reviewAreas,
                report: partialText,
              } satisfies ReviewDetails,
            });
          }
        }
      });

      try {
        await session.prompt(prompt);
        const report = getLastAssistantText(session.messages) || partialText.trim();
        if (!report) {
          throw new Error("Reviewer returned no report.");
        }

        const verdict = extractOverallVerdict(report);
        const header = [
          `Adversarial review complete.`,
          `Reviewer: ${modelLabel(reviewer)}${reviewer.name ? ` (${reviewer.name})` : ""}`,
          `Target: ${targetDir}`,
          planFile ? `Plan/spec: ${planFile}` : `Plan/spec: none provided`,
          verdict ? `Overall verdict: ${verdict}` : undefined,
          "",
        ]
          .filter(Boolean)
          .join("\n");

        return {
          content: [{ type: "text", text: `${header}\n${report}`.trim() }],
          details: {
            reviewerModel: modelLabel(reviewer),
            reviewerLabel: reviewer.name,
            targetDir,
            planFile,
            reviewAreas,
            overallVerdict: verdict,
            report,
          } satisfies ReviewDetails,
        };
      } finally {
        if (signal) signal.removeEventListener("abort", abortReviewer);
        unsubscribe?.();
        session.dispose();
      }
    },
  });
}
