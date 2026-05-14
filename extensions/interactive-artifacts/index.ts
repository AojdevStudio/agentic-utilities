import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { join } from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, formatSize, truncateHead } from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";

type ArtifactKind = "concept-explainer";

type ArtifactSection = {
  id: string;
  title: string;
  body: string;
  bullets: string[];
  examples: string[];
  questions: string[];
};

type ArtifactGlossaryTerm = {
  id: string;
  term: string;
  definition: string;
};

type ArtifactDocument = {
  artifactId: string;
  kind: ArtifactKind;
  title: string;
  subtitle: string;
  summary: string;
  sections: ArtifactSection[];
  glossary: ArtifactGlossaryTerm[];
  nextActions: string[];
  revision: number;
  updatedAt: number;
};

type ArtifactComment = {
  id: string;
  artifactId: string;
  nodeId: string;
  comment: string;
  createdAt: number;
  status: "queued" | "published";
};

type ArtifactRecord = {
  document: ArtifactDocument;
  comments: ArtifactComment[];
  token: string;
  busy: boolean;
  lastEvent: string;
};

type ArtifactPublishDetails = {
  artifactId: string;
  url: string;
  document: ArtifactDocument;
  commentCount: number;
  busy: boolean;
};

type CommentEntryData = {
  artifactId: string;
  commentId: string;
  nodeId: string;
  comment: string;
  createdAt: number;
};

type ActiveArtifactEntryData = {
  artifactId: string;
};

type CommentRequestBody = {
  nodeId: string;
  comment: string;
};

type ArtifactBootstrap = {
  artifactId: string;
  token: string;
  apiBase: string;
};

const ROUTE_PREFIX = "/interactive-artifacts";
const MAX_REQUEST_BODY_BYTES = 64 * 1024;
const MAX_COMMENT_LENGTH = 1200;
const COMMENT_ENTRY_TYPE = "interactive-artifact-comment";
const ACTIVE_ENTRY_TYPE = "interactive-artifact-active";
const EXTENSION_STATUS_KEY = "interactive-artifacts";
const EXTENSION_DIR = __dirname;
const PUBLIC_DIR = join(EXTENSION_DIR, "public");

const appJs = readStaticAsset("app.js");
const appCss = readStaticAsset("app.css");

const SectionInputSchema = Type.Object({
  id: Type.Optional(
    Type.String({ description: "Stable section id. Optional; the extension can generate it from the title." }),
  ),
  title: Type.String({ description: "Short section title." }),
  body: Type.String({
    description: "Concise markdown-like section body. Supports fenced ```mermaid code blocks for diagrams.",
  }),
  bullets: Type.Optional(Type.Array(Type.String(), { description: "Optional key bullets." })),
  examples: Type.Optional(Type.Array(Type.String(), { description: "Optional concrete examples." })),
  questions: Type.Optional(Type.Array(Type.String(), { description: "Optional review questions." })),
});

const GlossaryInputSchema = Type.Object({
  id: Type.Optional(Type.String({ description: "Stable glossary term id. Optional." })),
  term: Type.String({ description: "Glossary term." }),
  definition: Type.String({ description: "Glossary definition." }),
});

const ArtifactPublishParams = Type.Object({
  artifactId: Type.Optional(Type.String({ description: "Artifact id to update. Omit to use the active artifact." })),
  kind: Type.Optional(Type.String({ description: "Artifact kind. Use 'concept-explainer'." })),
  title: Type.String({ description: "Artifact title." }),
  subtitle: Type.Optional(Type.String({ description: "Optional subtitle." })),
  summary: Type.Optional(Type.String({ description: "Optional short overview shown near the top." })),
  sections: Type.Array(SectionInputSchema, { description: "Main explainer sections." }),
  glossary: Type.Optional(Type.Array(GlossaryInputSchema, { description: "Optional glossary terms." })),
  nextActions: Type.Optional(Type.Array(Type.String(), { description: "Optional next steps or takeaways." })),
});

const ArtifactGetParams = Type.Object({
  artifactId: Type.Optional(Type.String({ description: "Artifact id to fetch. Omit to use the active artifact." })),
});

function readStaticAsset(name: string): string {
  const path = join(PUBLIC_DIR, name);
  if (!existsSync(path)) {
    throw new Error(`Missing interactive-artifacts asset: ${path}`);
  }
  return readFileSync(path, "utf8");
}

function slugify(value: string): string {
  return (
    value
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 48) || "item"
  );
}

function trimText(value: string | undefined): string {
  return (value ?? "").trim();
}

function normalizeTextArray(values: string[] | undefined): string[] {
  return (values ?? []).map((value) => value.trim()).filter(Boolean);
}

function generateArtifactId(seed: string): string {
  return `artifact-${slugify(seed).slice(0, 24)}-${randomUUID().slice(0, 8)}`;
}

function normalizeArtifactId(candidate: string | undefined, fallbackSeed: string): string | null {
  const trimmed = candidate?.trim();
  if (!trimmed) return null;
  const safe = trimmed.replace(/^@+/, "");
  if (/^[a-zA-Z0-9_-]{1,80}$/.test(safe)) return safe;
  return `artifact-${slugify(safe || fallbackSeed).slice(0, 48)}`;
}

function createPlaceholderDocument(artifactId: string, topic: string): ArtifactDocument {
  return {
    artifactId,
    kind: "concept-explainer",
    title: `Building: ${topic}`,
    subtitle: "Pi is generating the first explainer revision.",
    summary: "The browser artifact is live. Pi will publish the first revision here in a moment.",
    sections: [],
    glossary: [],
    nextActions: [
      "Wait for the first revision to publish.",
      "Click a section and pin feedback once the explainer appears.",
    ],
    revision: 0,
    updatedAt: Date.now(),
  };
}

function normalizeDocument(
  input: {
    artifactId?: string;
    title: string;
    subtitle?: string;
    summary?: string;
    sections: Array<{
      id?: string;
      title: string;
      body: string;
      bullets?: string[];
      examples?: string[];
      questions?: string[];
    }>;
    glossary?: Array<{ id?: string; term: string; definition: string }>;
    nextActions?: string[];
  },
  artifactId: string,
  previousRevision: number,
): ArtifactDocument {
  const usedIds = new Set<string>();
  const uniqueId = (raw: string, fallbackPrefix: string, index: number): string => {
    const base = slugify(raw) || `${fallbackPrefix}-${index + 1}`;
    let candidate = base;
    let counter = 2;
    while (usedIds.has(candidate)) {
      candidate = `${base}-${counter++}`;
    }
    usedIds.add(candidate);
    return candidate;
  };

  const sections = input.sections.map((section, index) => ({
    id: uniqueId(section.id ?? section.title, "section", index),
    title: trimText(section.title) || `Section ${index + 1}`,
    body: trimText(section.body),
    bullets: normalizeTextArray(section.bullets),
    examples: normalizeTextArray(section.examples),
    questions: normalizeTextArray(section.questions),
  }));

  const glossary = (input.glossary ?? []).map((term, index) => ({
    id: uniqueId(term.id ?? term.term, "term", index),
    term: trimText(term.term) || `Term ${index + 1}`,
    definition: trimText(term.definition),
  }));

  return {
    artifactId,
    kind: "concept-explainer",
    title: trimText(input.title) || "Untitled explainer",
    subtitle: trimText(input.subtitle),
    summary: trimText(input.summary),
    sections,
    glossary,
    nextActions: normalizeTextArray(input.nextActions),
    revision: previousRevision + 1,
    updatedAt: Date.now(),
  };
}

function createArtifactUrl(port: number | null, artifactId: string): string {
  if (!port) throw new Error("Interactive artifact server is not running");
  return `http://127.0.0.1:${port}${ROUTE_PREFIX}/artifacts/${encodeURIComponent(artifactId)}`;
}

function createApiBase(port: number): string {
  return `http://127.0.0.1:${port}${ROUTE_PREFIX}/api`;
}

function openUrl(url: string): void {
  try {
    const platform = process.platform;
    const child =
      platform === "darwin"
        ? spawn("open", [url], { stdio: "ignore", detached: true })
        : platform === "win32"
          ? spawn("explorer", [url], { stdio: "ignore", detached: true, windowsHide: true })
          : spawn("xdg-open", [url], { stdio: "ignore", detached: true });
    child.on("error", () => {});
    child.unref();
  } catch {
    // Ignore browser-launch errors.
  }
}

function safeJsonForInlineScript(value: unknown): string {
  return JSON.stringify(value)
    .replace(/</g, "\\u003c")
    .replace(/>/g, "\\u003e")
    .replace(/&/g, "\\u0026")
    .replace(/\u2028/g, "\\u2028")
    .replace(/\u2029/g, "\\u2029");
}

function buildPageHtml(bootstrap: ArtifactBootstrap): string {
  const bootstrapJson = safeJsonForInlineScript(bootstrap);
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pi Interactive Artifact</title>
  <link rel="stylesheet" href="${ROUTE_PREFIX}/assets/app.css" />
</head>
<body>
  <div id="app"></div>
  <script>window.PI_ARTIFACT_BOOTSTRAP = ${bootstrapJson};</script>
  <script src="${ROUTE_PREFIX}/assets/app.js"></script>
</body>
</html>`;
}

export default function interactiveArtifacts(pi: ExtensionAPI) {
  let currentCtx: ExtensionContext | undefined;
  let server: Server | null = null;
  let serverPort: number | null = null;
  let agentBusy = false;
  let activeArtifactId: string | null = null;
  const artifacts = new Map<string, ArtifactRecord>();
  const subscribers = new Map<string, Set<ServerResponse>>();

  function updateStatus(): void {
    if (!currentCtx?.hasUI) return;
    if (!serverPort) {
      currentCtx.ui.setStatus(EXTENSION_STATUS_KEY, undefined);
      return;
    }
    const artifactCount = artifacts.size;
    const suffix = artifactCount > 0 ? ` • ${artifactCount} artifact${artifactCount === 1 ? "" : "s"}` : "";
    currentCtx.ui.setStatus(EXTENSION_STATUS_KEY, currentCtx.ui.theme.fg("accent", `🧩 ${serverPort}${suffix}`));
  }

  function notify(level: "info" | "warning" | "error", message: string): void {
    if (currentCtx?.hasUI) {
      currentCtx.ui.notify(message, level);
    }
  }

  function ensureArtifactRecord(artifactId: string, title?: string): ArtifactRecord {
    const existing = artifacts.get(artifactId);
    if (existing) return existing;
    const record: ArtifactRecord = {
      document: createPlaceholderDocument(artifactId, title ?? artifactId),
      comments: [],
      token: randomUUID(),
      busy: false,
      lastEvent: "Artifact created.",
    };
    artifacts.set(artifactId, record);
    updateStatus();
    return record;
  }

  function serializeArtifact(record: ArtifactRecord) {
    return {
      document: record.document,
      comments: record.comments,
      busy: record.busy,
      lastEvent: record.lastEvent,
    };
  }

  function emitArtifactUpdate(artifactId: string): void {
    const record = artifacts.get(artifactId);
    if (!record) return;
    const listeners = subscribers.get(artifactId);
    if (!listeners || listeners.size === 0) return;
    const payload = `data: ${JSON.stringify(serializeArtifact(record))}\n\n`;
    for (const response of listeners) {
      if (response.writableEnded) continue;
      response.write(payload);
    }
  }

  function resolveActiveArtifactId(): string | null {
    if (activeArtifactId && artifacts.has(activeArtifactId)) return activeArtifactId;
    const lastKey = Array.from(artifacts.keys()).at(-1) ?? null;
    activeArtifactId = lastKey;
    return lastKey;
  }

  function persistActiveArtifact(artifactId: string): void {
    activeArtifactId = artifactId;
    pi.appendEntry(ACTIVE_ENTRY_TYPE, { artifactId } satisfies ActiveArtifactEntryData);
  }

  function buildKickoffPrompt(topic: string, artifactId: string): string {
    return [
      `Create an interactive concept explainer artifact about \"${topic}\".`,
      "",
      "Use the interactive-artifact tools, not plain chat, for the artifact itself:",
      "1. If you need more context, inspect files or reason first.",
      "2. Publish the full explainer with artifact_publish.",
      "3. Keep future updates full-state: use artifact_get, then artifact_publish again.",
      "",
      `Artifact id: ${artifactId}`,
      "Constraints:",
      "- kind: concept-explainer",
      "- 4-7 sections",
      "- concise section bodies",
      "- include bullets, examples, and review questions when helpful",
      "- stable ids derived from titles are fine",
      "",
      "After publishing, give a brief chat summary of what you created.",
    ].join("\n");
  }

  function buildCommentPrompt(comment: ArtifactComment): string {
    return [
      `A pinned browser comment arrived for interactive artifact ${comment.artifactId}.`,
      "",
      `Node: ${comment.nodeId}`,
      `Comment: ${comment.comment}`,
      "",
      "Use artifact_get to inspect the latest artifact and recent comments, then update the full artifact with artifact_publish.",
      "Address the feedback directly, keep the explainer tight, and preserve anything already working well.",
    ].join("\n");
  }

  async function dispatchUserMessage(prompt: string): Promise<void> {
    try {
      if (agentBusy) {
        pi.sendUserMessage(prompt, { deliverAs: "followUp" });
        notify("info", "Agent busy. Feedback queued as a follow-up.");
        return;
      }
      pi.sendUserMessage(prompt);
    } catch {
      pi.sendUserMessage(prompt, { deliverAs: "followUp" });
    }
  }

  function parseCommentBody(body: string): CommentRequestBody {
    const parsed = JSON.parse(body) as Partial<CommentRequestBody>;
    const nodeId = typeof parsed.nodeId === "string" ? parsed.nodeId.trim() : "";
    const comment = typeof parsed.comment === "string" ? parsed.comment.trim() : "";
    if (!nodeId) throw new Error("Missing nodeId");
    if (!comment) throw new Error("Missing comment");
    if (comment.length > MAX_COMMENT_LENGTH) throw new Error(`Comment exceeds ${MAX_COMMENT_LENGTH} characters`);
    return { nodeId, comment };
  }

  function readRequestBody(req: IncomingMessage): Promise<string> {
    return new Promise((resolve, reject) => {
      let body = "";
      let size = 0;
      req.on("data", (chunk: Buffer | string) => {
        size += Buffer.byteLength(chunk);
        if (size > MAX_REQUEST_BODY_BYTES) {
          reject(new Error("Request body too large"));
          req.destroy();
          return;
        }
        body += chunk.toString();
      });
      req.on("end", () => resolve(body));
      req.on("error", reject);
    });
  }

  function sendJson(res: ServerResponse, status: number, payload: unknown): void {
    if (res.writableEnded) return;
    res.writeHead(status, {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Frame-Options": "DENY",
    });
    res.end(JSON.stringify(payload));
  }

  function sendHtml(res: ServerResponse, html: string): void {
    res.writeHead(200, {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      "Content-Security-Policy":
        "default-src 'self'; connect-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; base-uri 'none'; frame-ancestors 'none'",
      "X-Frame-Options": "DENY",
    });
    res.end(html);
  }

  function sendStatic(res: ServerResponse, contentType: string, body: string): void {
    res.writeHead(200, {
      "Content-Type": contentType,
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
    });
    res.end(body);
  }

  async function handleCommentPost(artifactId: string, req: IncomingMessage, res: ServerResponse): Promise<void> {
    const record = artifacts.get(artifactId);
    if (!record) {
      sendJson(res, 404, { ok: false, error: "Unknown artifact" });
      return;
    }

    const tokenHeaderRaw = req.headers["x-pi-artifact-token"];
    const tokenHeader = Array.isArray(tokenHeaderRaw) ? tokenHeaderRaw[0] : tokenHeaderRaw;
    if (tokenHeader !== record.token) {
      sendJson(res, 403, { ok: false, error: "Invalid artifact token" });
      return;
    }

    let parsed: CommentRequestBody;
    try {
      parsed = parseCommentBody(await readRequestBody(req));
    } catch (error) {
      sendJson(res, 400, { ok: false, error: error instanceof Error ? error.message : "Invalid request" });
      return;
    }

    const comment: ArtifactComment = {
      id: randomUUID(),
      artifactId,
      nodeId: parsed.nodeId,
      comment: parsed.comment,
      createdAt: Date.now(),
      status: "queued",
    };

    record.comments.push(comment);
    record.busy = true;
    record.lastEvent = `Queued feedback for ${parsed.nodeId}.`;
    persistActiveArtifact(artifactId);
    pi.appendEntry(COMMENT_ENTRY_TYPE, {
      artifactId,
      commentId: comment.id,
      nodeId: parsed.nodeId,
      comment: parsed.comment,
      createdAt: comment.createdAt,
    } satisfies CommentEntryData);
    emitArtifactUpdate(artifactId);
    notify("info", `Pinned feedback queued for ${parsed.nodeId}.`);
    await dispatchUserMessage(buildCommentPrompt(comment));
    sendJson(res, 200, { ok: true });
  }

  function handleSse(artifactId: string, token: string, res: ServerResponse): void {
    const record = artifacts.get(artifactId);
    if (!record) {
      sendJson(res, 404, { ok: false, error: "Unknown artifact" });
      return;
    }
    if (token !== record.token) {
      sendJson(res, 403, { ok: false, error: "Invalid artifact token" });
      return;
    }

    res.writeHead(200, {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-store",
      Connection: "keep-alive",
    });
    res.write(`data: ${JSON.stringify(serializeArtifact(record))}\n\n`);

    let listenerSet = subscribers.get(artifactId);
    if (!listenerSet) {
      listenerSet = new Set();
      subscribers.set(artifactId, listenerSet);
    }
    listenerSet.add(res);

    const cleanup = () => {
      const set = subscribers.get(artifactId);
      if (!set) return;
      set.delete(res);
      if (set.size === 0) subscribers.delete(artifactId);
    };
    res.on("close", cleanup);
    res.on("finish", cleanup);
  }

  function handleRequest(req: IncomingMessage, res: ServerResponse): void {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    const pathname = url.pathname;

    if (req.method === "GET" && pathname === `${ROUTE_PREFIX}/assets/app.js`) {
      sendStatic(res, "application/javascript; charset=utf-8", appJs);
      return;
    }
    if (req.method === "GET" && pathname === `${ROUTE_PREFIX}/assets/app.css`) {
      sendStatic(res, "text/css; charset=utf-8", appCss);
      return;
    }

    const pageMatch = pathname.match(/^\/interactive-artifacts\/artifacts\/([^/]+)$/);
    if (req.method === "GET" && pageMatch) {
      const artifactId = decodeURIComponent(pageMatch[1] ?? "");
      const record = artifacts.get(artifactId);
      if (!record || !serverPort) {
        sendJson(res, 404, { ok: false, error: "Unknown artifact" });
        return;
      }
      sendHtml(res, buildPageHtml({ artifactId, token: record.token, apiBase: createApiBase(serverPort) }));
      return;
    }

    const artifactJsonMatch = pathname.match(/^\/interactive-artifacts\/api\/artifacts\/([^/]+)$/);
    if (req.method === "GET" && artifactJsonMatch) {
      const artifactId = decodeURIComponent(artifactJsonMatch[1] ?? "");
      const record = artifacts.get(artifactId);
      if (!record) {
        sendJson(res, 404, { ok: false, error: "Unknown artifact" });
        return;
      }
      sendJson(res, 200, serializeArtifact(record));
      return;
    }

    const streamMatch = pathname.match(/^\/interactive-artifacts\/api\/artifacts\/([^/]+)\/stream$/);
    if (req.method === "GET" && streamMatch) {
      const artifactId = decodeURIComponent(streamMatch[1] ?? "");
      handleSse(artifactId, url.searchParams.get("token") ?? "", res);
      return;
    }

    const commentMatch = pathname.match(/^\/interactive-artifacts\/api\/artifacts\/([^/]+)\/comments$/);
    if (req.method === "POST" && commentMatch) {
      void handleCommentPost(decodeURIComponent(commentMatch[1] ?? ""), req, res);
      return;
    }

    sendJson(res, 404, { ok: false, error: "Not found" });
  }

  async function ensureServer(ctx?: ExtensionContext): Promise<number> {
    if (serverPort) return serverPort;
    server = createServer(handleRequest);
    serverPort = await new Promise<number>((resolve, reject) => {
      const onError = (error: Error) => {
        server?.off("error", onError);
        reject(error);
      };
      server?.once("error", onError);
      server?.listen(0, "127.0.0.1", () => {
        server?.off("error", onError);
        const address = server?.address();
        if (address && typeof address === "object") {
          resolve(address.port);
          return;
        }
        reject(new Error("Could not determine interactive-artifact server port"));
      });
    });
    updateStatus();
    ctx?.ui.notify(`Interactive artifacts server ready on http://127.0.0.1:${serverPort}`, "info");
    for (const artifactId of artifacts.keys()) emitArtifactUpdate(artifactId);
    return serverPort;
  }

  async function closeServer(): Promise<void> {
    for (const listenerSet of subscribers.values()) {
      for (const response of listenerSet) {
        try {
          response.end();
        } catch {
          // ignore
        }
      }
    }
    subscribers.clear();
    if (!server) {
      serverPort = null;
      updateStatus();
      return;
    }
    await new Promise<void>((resolve) => {
      server?.close(() => resolve());
    });
    server = null;
    serverPort = null;
    updateStatus();
  }

  function reconstructState(ctx: ExtensionContext): void {
    const previousArtifacts = Array.from(artifacts.keys());
    const previousRecords = new Map(artifacts);
    artifacts.clear();
    activeArtifactId = null;
    agentBusy = false;

    for (const entry of ctx.sessionManager.getBranch()) {
      if (entry.type === "custom") {
        if (entry.customType === ACTIVE_ENTRY_TYPE) {
          const data = entry.data as Partial<ActiveArtifactEntryData> | undefined;
          if (typeof data?.artifactId === "string") activeArtifactId = data.artifactId;
        }
        if (entry.customType === COMMENT_ENTRY_TYPE) {
          const data = entry.data as Partial<CommentEntryData> | undefined;
          if (!data || typeof data.artifactId !== "string" || typeof data.comment !== "string") continue;
          const record = ensureArtifactRecord(data.artifactId, previousRecords.get(data.artifactId)?.document.title);
          record.comments.push({
            id: typeof data.commentId === "string" ? data.commentId : randomUUID(),
            artifactId: data.artifactId,
            nodeId: typeof data.nodeId === "string" ? data.nodeId : "unknown",
            comment: data.comment,
            createdAt: typeof data.createdAt === "number" ? data.createdAt : Date.now(),
            status: "published",
          });
        }
        continue;
      }

      if (entry.type !== "message") continue;
      const message = entry.message;
      if (message.role !== "toolResult" || message.toolName !== "artifact_publish") continue;
      const details = message.details as ArtifactPublishDetails | undefined;
      if (!details?.document?.artifactId) continue;
      const existing = artifacts.get(details.document.artifactId);
      const previous = previousRecords.get(details.document.artifactId);
      artifacts.set(details.document.artifactId, {
        document: details.document,
        comments: existing?.comments ?? previous?.comments ?? [],
        token: existing?.token ?? previous?.token ?? randomUUID(),
        busy: false,
        lastEvent: `Revision ${details.document.revision} published.`,
      });
      activeArtifactId = details.document.artifactId;
    }

    if (!activeArtifactId) {
      activeArtifactId = Array.from(artifacts.keys()).at(-1) ?? null;
    }

    for (const record of artifacts.values()) {
      const hasPublishedRevision = record.document.revision > 0;
      record.comments = record.comments.map((comment) => ({
        ...comment,
        status: hasPublishedRevision && comment.createdAt <= record.document.updatedAt ? "published" : "queued",
      }));
      record.busy = record.comments.some((comment) => comment.status === "queued");
      if (record.busy) {
        record.lastEvent = "Feedback is queued for Pi.";
      }
    }

    for (const artifactId of previousArtifacts) emitArtifactUpdate(artifactId);
    for (const artifactId of artifacts.keys()) emitArtifactUpdate(artifactId);
    updateStatus();
  }

  pi.on("session_start", async (_event, ctx) => {
    currentCtx = ctx;
    reconstructState(ctx);
  });

  pi.on("session_tree", async (_event, ctx) => {
    currentCtx = ctx;
    reconstructState(ctx);
  });

  pi.on("session_shutdown", async () => {
    currentCtx = undefined;
    agentBusy = false;
    await closeServer();
  });

  pi.on("agent_start", async () => {
    agentBusy = true;
  });

  pi.on("agent_end", async () => {
    agentBusy = false;
  });

  pi.registerCommand("artifact-explain", {
    description: "Open a browser artifact and ask pi to build a concept explainer in it",
    handler: async (args, ctx) => {
      const topic = args.trim();
      if (!topic) {
        ctx.ui.notify("Usage: /artifact-explain <topic>", "warning");
        return;
      }

      const port = await ensureServer(ctx);
      const artifactId = generateArtifactId(topic);
      const record = ensureArtifactRecord(artifactId, topic);
      record.document = createPlaceholderDocument(artifactId, topic);
      record.busy = true;
      record.lastEvent = "Pi is generating the first revision.";
      persistActiveArtifact(artifactId);
      emitArtifactUpdate(artifactId);

      const url = createArtifactUrl(port, artifactId);
      openUrl(url);
      ctx.ui.notify(`Artifact opened in browser: ${url}`, "info");
      await dispatchUserMessage(buildKickoffPrompt(topic, artifactId));
    },
  });

  pi.registerCommand("artifact-open", {
    description: "Open the current interactive artifact in your browser",
    getArgumentCompletions: (prefix) => {
      const items = Array.from(artifacts.entries())
        .filter(
          ([artifactId, record]) =>
            artifactId.startsWith(prefix) || record.document.title.toLowerCase().includes(prefix.toLowerCase()),
        )
        .map(([artifactId, record]) => ({ value: artifactId, label: artifactId, description: record.document.title }));
      return items.length > 0 ? items : null;
    },
    handler: async (args, ctx) => {
      const artifactId = args.trim() || resolveActiveArtifactId();
      if (!artifactId) {
        ctx.ui.notify("No interactive artifacts exist in this session yet.", "warning");
        return;
      }
      if (!artifacts.has(artifactId)) {
        ctx.ui.notify(`Unknown artifact: ${artifactId}`, "error");
        return;
      }
      persistActiveArtifact(artifactId);
      const port = await ensureServer(ctx);
      const url = createArtifactUrl(port, artifactId);
      openUrl(url);
      ctx.ui.notify(`Opened ${artifactId}`, "info");
    },
  });

  pi.registerTool({
    name: "artifact_publish",
    label: "Artifact Publish",
    description: "Publish or update the full browser-based interactive artifact state.",
    promptSnippet: "Publish or update the browser-based interactive artifact for concept explainers.",
    promptGuidelines: [
      "Use artifact_publish to create or revise the browser-based explainer artifact instead of describing the artifact only in chat.",
      "Use artifact_publish section bodies with fenced ```mermaid blocks when diagrams make workflows or architecture easier to understand; the browser artifact renders Mermaid diagrams.",
      "Use artifact_publish with the full artifact state each time, not a partial patch.",
      "Use artifact_get before revising an existing artifact so the new artifact_publish call preserves the good parts of the current revision.",
    ],
    parameters: ArtifactPublishParams,
    async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
      const port = await ensureServer(ctx);
      const artifactId =
        normalizeArtifactId(params.artifactId, params.title) ||
        resolveActiveArtifactId() ||
        generateArtifactId(params.title);
      const record = ensureArtifactRecord(artifactId, params.title);
      const document = normalizeDocument(
        {
          artifactId,
          title: params.title,
          subtitle: params.subtitle,
          summary: params.summary,
          sections: params.sections,
          glossary: params.glossary,
          nextActions: params.nextActions,
        },
        artifactId,
        record.document.revision,
      );
      record.document = document;
      record.busy = false;
      record.lastEvent = `Revision ${document.revision} published.`;
      record.comments = record.comments.map((comment) => ({ ...comment, status: "published" }));
      persistActiveArtifact(artifactId);
      emitArtifactUpdate(artifactId);
      const url = createArtifactUrl(port, artifactId);
      return {
        content: [
          {
            type: "text",
            text: `Published interactive artifact \"${document.title}\" (rev ${document.revision}) at ${url}`,
          },
        ],
        details: {
          artifactId,
          url,
          document,
          commentCount: record.comments.length,
          busy: false,
        } satisfies ArtifactPublishDetails,
      };
    },
  });

  pi.registerTool({
    name: "artifact_get",
    label: "Artifact Get",
    description: "Get the current interactive artifact JSON and recent pinned comments.",
    promptSnippet: "Fetch the current browser-artifact JSON and recent pinned comments before revising the explainer.",
    promptGuidelines: [
      "Use artifact_get before artifact_publish when revising an existing browser artifact from pinned feedback.",
    ],
    parameters: ArtifactGetParams,
    async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
      const artifactId = trimText(params.artifactId) || resolveActiveArtifactId();
      if (!artifactId) throw new Error("No interactive artifact exists in this session yet.");
      const record = artifacts.get(artifactId);
      if (!record) throw new Error(`Unknown artifact: ${artifactId}`);
      const url = createArtifactUrl(serverPort ?? (await ensureServer(ctx)), artifactId);
      const snapshot = {
        artifactId,
        url,
        busy: record.busy,
        lastEvent: record.lastEvent,
        document: record.document,
        comments: record.comments.slice(-12),
      };
      const fullJson = JSON.stringify(snapshot, null, 2);
      const truncation = truncateHead(fullJson, {
        maxLines: DEFAULT_MAX_LINES,
        maxBytes: DEFAULT_MAX_BYTES,
      });
      let text = `Current interactive artifact state:\n\n\`\`\`json\n${truncation.content}\n\`\`\``;
      if (truncation.truncated) {
        text += `\n[Truncated: ${truncation.outputLines} of ${truncation.totalLines} lines (${formatSize(truncation.outputBytes)} of ${formatSize(truncation.totalBytes)})]`;
      }
      return {
        content: [{ type: "text", text }],
        details: snapshot,
      };
    },
  });
}
