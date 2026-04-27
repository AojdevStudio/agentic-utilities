import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";

const PACKAGE_NAME = "agentic-utilities";
const VERSION = "0.1.0";

export default function (pi: ExtensionAPI) {
  pi.registerCommand("agentic-utilities", {
    description: "Show agentic-utilities package status",
    handler: async (_args, ctx) => {
      const message = `${PACKAGE_NAME} ${VERSION} loaded`;
      if (ctx.hasUI) ctx.ui.notify(message, "info");
    },
  });

  pi.registerTool({
    name: "agentic_utilities_ping",
    label: "Agentic Utilities Ping",
    description: "Check that the agentic-utilities Pi package is loaded.",
    promptSnippet: "Check whether the agentic-utilities Pi package is loaded.",
    promptGuidelines: [
      "Use agentic_utilities_ping only when verifying that the agentic-utilities package is installed or loaded.",
    ],
    parameters: Type.Object({
      message: Type.Optional(Type.String({ description: "Optional text to echo back." })),
    }),
    async execute(_toolCallId, params) {
      const suffix = params.message ? `: ${params.message}` : ".";
      return {
        content: [{ type: "text", text: `${PACKAGE_NAME} loaded${suffix}` }],
        details: {
          ok: true,
          package: PACKAGE_NAME,
          version: VERSION,
        },
      };
    },
  });
}
