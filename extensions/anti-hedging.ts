import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

const ANTI_HEDGING_RULES = `
Execution style rules:
- Do not end responses with optional-offer filler such as "if you want, I can...", "I can also...", "let me know if you'd like me to...", or similar phrasing.
- If an obvious, low-risk, high-value follow-up can be done in the same turn, do it immediately instead of offering it.
- Prefer this sequence: inspect first, answer directly, include the most useful verification, comparison, or next obvious result automatically.
- Ask follow-up questions only when required information is genuinely missing, the request is materially ambiguous, or the action would be destructive, external, or irreversible.
- Avoid permission-seeking phrasing for non-destructive local inspection.
- Be decisive. Replace soft offers with completed work whenever possible.
- When citing findings from tools or files, include the strongest relevant conclusion up front, then supporting evidence.
- Do not pad endings with generic assistance offers. End after delivering the answer or concrete next actions.
`;

const HEDGING_PATTERNS = [
  /if you want,? i can/i,
  /i can also /i,
  /let me know if you'd like me to/i,
  /if you'd like,? i can/i,
  /i can do that for you/i,
];

export default function antiHedgingExtension(pi: ExtensionAPI) {
  pi.on("before_agent_start", async (event) => {
    return {
      systemPrompt: `${event.systemPrompt}\n\n${ANTI_HEDGING_RULES}`,
    };
  });

  pi.on("message_end", async (event) => {
    const message = event.message;
    if (!message || message.role !== "assistant") return;

    const content =
      typeof message.content === "string"
        ? message.content
        : Array.isArray(message.content)
          ? message.content
              .map((item: any) => (item?.type === "text" ? item.text : ""))
              .filter(Boolean)
              .join("\n")
          : "";

    if (!content) return;
    if (!HEDGING_PATTERNS.some((pattern) => pattern.test(content))) return;

    pi.sendMessage({
      customType: "anti-hedging-warning",
      content:
        "Style warning: assistant response contained optional-offer filler instead of proactively completing the obvious next step.",
      display: true,
      details: {
        timestamp: Date.now(),
      },
    });
  });

  pi.registerCommand("antihedging", {
    description: "Show the active anti-hedging behavior rules",
    handler: async (_args, ctx) => {
      ctx.ui.notify("Anti-hedging extension is active globally.", "info");
      pi.sendMessage({
        customType: "anti-hedging-rules",
        content: ANTI_HEDGING_RULES.trim(),
        display: true,
        details: { timestamp: Date.now() },
      });
    },
  });
}
