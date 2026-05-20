import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, formatSize, truncateHead } from "@mariozechner/pi-coding-agent";
import { Type } from "typebox";

const BRAVE_API_URL = "https://api.search.brave.com/res/v1/web/search";
const GOOGLE_CSE_API_URL = "https://www.googleapis.com/customsearch/v1";
const TAVILY_API_BASE = "https://api.tavily.com";
const ENV_FILE = join(homedir(), ".env");

type Json = Record<string, any>;

let cachedEnv: Record<string, string> | null = null;

function parseEnvFile(): Record<string, string> {
  if (cachedEnv) return cachedEnv;
  const env: Record<string, string> = {};

  if (!existsSync(ENV_FILE)) {
    cachedEnv = env;
    return env;
  }

  const content = readFileSync(ENV_FILE, "utf8");
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;

    const normalized = line.startsWith("export ") ? line.slice(7) : line;
    const idx = normalized.indexOf("=");
    if (idx === -1) continue;

    const key = normalized.slice(0, idx).trim();
    let value = normalized.slice(idx + 1).trim();

    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }

    env[key] = value;
  }

  cachedEnv = env;
  return env;
}

function getOptionalSecret(...names: string[]): string | undefined {
  const envFile = parseEnvFile();
  for (const name of names) {
    const value = process.env[name] || envFile[name];
    if (value) return value;
  }
  return undefined;
}

function getSecret(name: string): string {
  const value = getOptionalSecret(name);
  if (!value) {
    throw new Error(`Missing ${name}. Add it to your shell environment or ${ENV_FILE}.`);
  }
  return value;
}

function toText(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value, null, 2);
}

function makeTextResult(value: unknown, details: Json = {}) {
  const fullText = toText(value);
  const truncation = truncateHead(fullText, {
    maxLines: DEFAULT_MAX_LINES,
    maxBytes: DEFAULT_MAX_BYTES,
  });

  let text = truncation.content;
  if (truncation.truncated) {
    text += `\n\n[Output truncated: showing ${truncation.outputLines} of ${truncation.totalLines} lines (${formatSize(truncation.outputBytes)} of ${formatSize(truncation.totalBytes)})]`;
  }

  return {
    content: [{ type: "text" as const, text }],
    details: { ...details, truncation },
  };
}

type StructuredSearchArgs = {
  query?: string;
  exactPhrases?: string[];
  excludeTerms?: string[];
  site?: string;
};

function stripWrappingQuotes(value: string): string {
  return value.length >= 2 && value.startsWith('"') && value.endsWith('"') ? value.slice(1, -1).trim() : value;
}

function cleanItems(values?: string[]): string[] {
  if (!values) return [];
  return values.map((value) => stripWrappingQuotes(value.trim().replace(/\s+/g, " "))).filter(Boolean);
}

function cleanQuery(value?: string): string | undefined {
  if (typeof value !== "string") return undefined;
  const cleaned = value.trim().replace(/\s+/g, " ");
  return cleaned || undefined;
}

function normalizeSite(site?: string): string | undefined {
  if (typeof site !== "string") return undefined;
  let value = site
    .trim()
    .replace(/^site:/i, "")
    .trim();
  if (!value) return undefined;

  try {
    const candidate = /^[a-z]+:\/\//i.test(value) ? value : `https://${value}`;
    const url = new URL(candidate);
    if (url.hostname) value = url.hostname;
  } catch {}

  return value.replace(/\/+$/, "") || undefined;
}

function quoteForSearch(value: string): string {
  return `"${value.replace(/"/g, '\\"')}"`;
}

function buildSearchQuery(args: StructuredSearchArgs): {
  query: string;
  site?: string;
} {
  const baseQuery = cleanQuery(args.query);
  const exactPhrases = cleanItems(args.exactPhrases);
  const excludeTerms = cleanItems(args.excludeTerms);
  const site = normalizeSite(args.site);

  if (!baseQuery && exactPhrases.length === 0) {
    throw new Error("At least one of query or exactPhrases is required.");
  }

  const parts: string[] = [];
  if (baseQuery) parts.push(baseQuery);
  for (const phrase of exactPhrases) parts.push(quoteForSearch(phrase));
  for (const term of excludeTerms) parts.push(`-${term.includes(" ") ? quoteForSearch(term) : term}`);
  if (site) parts.push(`site:${site}`);
  return { query: parts.join(" "), site };
}

function hasGoogleCredentials(): boolean {
  return Boolean(
    getOptionalSecret("GOOGLE_SEARCH_API_KEY", "GOOGLE_API_KEY") &&
      getOptionalSecret("GOOGLE_CSE_ID", "GOOGLE_CUSTOM_SEARCH_ENGINE_ID"),
  );
}

function formatSearchResults(
  results: Array<{ title: string; url: string; description: string }>,
  count: number,
): string {
  if (!results.length) return "No results found.";
  return results
    .slice(0, count)
    .map((r, i) => `${i + 1}. ${r.title}\n${r.url}\n${r.description}`.trim())
    .join("\n\n");
}

async function googleSearch(
  query: string,
  count: number,
): Promise<Array<{ title: string; url: string; description: string }>> {
  const apiKey = getOptionalSecret("GOOGLE_SEARCH_API_KEY", "GOOGLE_API_KEY");
  const cseId = getOptionalSecret("GOOGLE_CSE_ID", "GOOGLE_CUSTOM_SEARCH_ENGINE_ID");
  if (!apiKey || !cseId) {
    throw new Error(
      `Missing Google CSE credentials. Add GOOGLE_SEARCH_API_KEY and GOOGLE_CSE_ID to your shell environment or ${ENV_FILE}.`,
    );
  }

  const url = new URL(GOOGLE_CSE_API_URL);
  url.searchParams.set("key", apiKey);
  url.searchParams.set("cx", cseId);
  url.searchParams.set("q", query);
  url.searchParams.set("num", String(Math.min(Math.max(count, 1), 10)));

  const data = await fetchJson(url.toString(), {
    headers: { Accept: "application/json" },
  });
  return (data?.items ?? []).map((item: any) => ({
    title: item.title,
    url: item.link,
    description: item.snippet?.replace(/\n/g, " ") ?? "",
  }));
}

async function fetchJson(url: string, init: RequestInit): Promise<any> {
  const res = await fetch(url, init);
  const text = await res.text();
  let data: any = null;

  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = text;
  }

  if (!res.ok) {
    const message = typeof data === "string" ? data : JSON.stringify(data, null, 2);
    throw new Error(`HTTP ${res.status} ${res.statusText}: ${message}`);
  }

  return data;
}

async function tavilyPost(path: string, body: Json) {
  const apiKey = getSecret("TAVILY_API_KEY");
  return fetchJson(`${TAVILY_API_BASE}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify(body),
  });
}

export default function (pi: ExtensionAPI) {
  pi.registerTool({
    name: "web_search",
    label: "Web Search",
    description:
      "Search the public web. Uses Brave Search by default, with optional Google Custom Search fallback/backend when Google CSE credentials are configured.",
    parameters: Type.Object({
      query: Type.Optional(Type.String({ description: "Search query" })),
      exactPhrases: Type.Optional(
        Type.Array(Type.String(), {
          description: "Exact phrases to match; each becomes a quoted phrase",
        }),
      ),
      excludeTerms: Type.Optional(
        Type.Array(Type.String(), {
          description: "Terms or phrases to exclude",
        }),
      ),
      site: Type.Optional(
        Type.String({
          description: "Optional site/domain restriction, e.g. example.com",
        }),
      ),
      backend: Type.Optional(
        Type.String({
          description:
            "Search backend: brave (default) or google. Google CSE is used as fallback when Brave has no results and Google credentials exist.",
        }),
      ),
      count: Type.Optional(
        Type.Number({
          description: "Number of results to return (1-10)",
          default: 5,
        }),
      ),
      country: Type.Optional(Type.String({ description: "Country code, e.g. US" })),
      search_lang: Type.Optional(Type.String({ description: "Language code, e.g. en" })),
      safesearch: Type.Optional(
        Type.String({
          description: "Safe search level: off, moderate, or strict",
          default: "moderate",
        }),
      ),
      freshness: Type.Optional(Type.String({ description: "Freshness window such as pd, pw, pm, py" })),
    }),
    async execute(_toolCallId, params) {
      const count = Math.min(Math.max(params.count ?? 5, 1), 10);
      const built = buildSearchQuery(params);
      const backend = (params.backend ?? "brave").toLowerCase();

      if (backend === "google") {
        const results = await googleSearch(built.query, count);
        return makeTextResult(formatSearchResults(results, count), {
          backend: "google",
          query: params.query,
          composedQuery: built.query,
          count,
          returned: results.length,
        });
      }

      let data: any;
      let braveError: unknown;
      try {
        const apiKey = getSecret("BRAVE_SEARCH_API_KEY");
        const url = new URL(BRAVE_API_URL);
        url.searchParams.set("q", built.query);
        url.searchParams.set("count", String(count));
        if (params.country) url.searchParams.set("country", params.country);
        if (params.search_lang) url.searchParams.set("search_lang", params.search_lang);
        if (params.safesearch) url.searchParams.set("safesearch", params.safesearch);
        if (params.freshness) url.searchParams.set("freshness", params.freshness);

        data = await fetchJson(url.toString(), {
          headers: {
            Accept: "application/json",
            "X-Subscription-Token": apiKey,
          },
        });
      } catch (error) {
        braveError = error;
      }

      const webResults = data?.web?.results ?? [];
      if ((!webResults.length || braveError) && hasGoogleCredentials()) {
        const googleResults = await googleSearch(built.query, count);
        return makeTextResult(formatSearchResults(googleResults, count), {
          backend: "google-fallback",
          fallbackReason: braveError
            ? braveError instanceof Error
              ? braveError.message
              : String(braveError)
            : "no Brave results",
          query: params.query,
          composedQuery: built.query,
          count,
          returned: googleResults.length,
        });
      }

      if (braveError) throw braveError;

      if (!webResults.length) {
        return makeTextResult(`No web results found for: ${built.query}`, {
          backend: "brave",
          query: params.query,
          composedQuery: built.query,
          count,
          returned: 0,
        });
      }

      const text = webResults
        .slice(0, count)
        .map((r: any, i: number) => {
          const age = r.age ? `\nAge: ${r.age}` : "";
          const meta = r.meta_url?.hostname ? `\nDomain: ${r.meta_url.hostname}` : "";
          return `${i + 1}. ${r.title}\n${r.url}${meta}${age}\n${r.description ?? ""}`.trim();
        })
        .join("\n\n");

      return makeTextResult(text, {
        backend: "brave",
        query: params.query,
        composedQuery: built.query,
        count,
        returned: webResults.length,
      });
    },
  });

  pi.registerTool({
    name: "tavily_search",
    label: "Tavily Search",
    description:
      "AI-native search with structured results. Best for research, retrieval, summaries, and cleaner agent-friendly search output.",
    parameters: Type.Object({
      query: Type.String({ description: "Search query" }),
      search_depth: Type.Optional(
        Type.String({
          description: "Search depth: basic, advanced, fast, or ultra-fast",
          default: "advanced",
        }),
      ),
      topic: Type.Optional(Type.String({ description: "Optional topic such as general or news" })),
      max_results: Type.Optional(Type.Number({ description: "Maximum results", default: 5 })),
      include_answer: Type.Optional(
        Type.Boolean({
          description: "Include Tavily-generated answer",
          default: true,
        }),
      ),
      include_raw_content: Type.Optional(
        Type.Boolean({
          description: "Include extracted raw content per result",
          default: false,
        }),
      ),
      include_images: Type.Optional(Type.Boolean({ description: "Include image results", default: false })),
      include_domains: Type.Optional(
        Type.Array(Type.String(), {
          description: "Only include these domains",
        }),
      ),
      exclude_domains: Type.Optional(Type.Array(Type.String(), { description: "Exclude these domains" })),
      exact_match: Type.Optional(
        Type.Boolean({
          description: "Require exact quoted phrase matches",
          default: false,
        }),
      ),
    }),
    async execute(_toolCallId, params) {
      const body = {
        query: params.query,
        search_depth: params.search_depth ?? "advanced",
        topic: params.topic,
        max_results: params.max_results ?? 5,
        include_answer: params.include_answer ?? true,
        include_raw_content: params.include_raw_content ?? false,
        include_images: params.include_images ?? false,
        include_domains: params.include_domains,
        exclude_domains: params.exclude_domains,
        exact_match: params.exact_match ?? false,
      };

      const data = await tavilyPost("/search", body);
      const results = data?.results ?? [];

      const sections: string[] = [];
      if (data?.answer) sections.push(`Answer:\n${data.answer}`);

      if (!results.length) {
        sections.push(`No Tavily search results found for: ${params.query}`);
      } else {
        sections.push(
          results
            .map((r: any, i: number) => {
              const score = r.score != null ? `\nScore: ${r.score}` : "";
              const raw = r.raw_content ? `\nRaw content:\n${r.raw_content}` : "";
              return `${i + 1}. ${r.title}\n${r.url}${score}\n${r.content ?? ""}${raw}`.trim();
            })
            .join("\n\n"),
        );
      }

      if (Array.isArray(data?.images) && data.images.length) {
        sections.push(`Images:\n${data.images.join("\n")}`);
      }

      return makeTextResult(sections.join("\n\n"), {
        query: params.query,
        returned: results.length,
        request_id: data?.request_id,
      });
    },
  });

  pi.registerTool({
    name: "tavily_crawl",
    label: "Tavily Crawl",
    description: "Crawl a site from a base URL and return fetched page content.",
    parameters: Type.Object({
      url: Type.String({ description: "Base URL to crawl" }),
      instructions: Type.Optional(Type.String({ description: "Natural-language crawl instructions" })),
      max_depth: Type.Optional(Type.Number({ description: "Maximum crawl depth", default: 1 })),
      max_breadth: Type.Optional(
        Type.Number({
          description: "Maximum links followed per level",
          default: 20,
        }),
      ),
      limit: Type.Optional(Type.Number({ description: "Total crawl result limit", default: 20 })),
      select_paths: Type.Optional(
        Type.Array(Type.String(), {
          description: "Regex path patterns to include",
        }),
      ),
      exclude_paths: Type.Optional(
        Type.Array(Type.String(), {
          description: "Regex path patterns to exclude",
        }),
      ),
      select_domains: Type.Optional(Type.Array(Type.String(), { description: "Regex domains to include" })),
      exclude_domains: Type.Optional(Type.Array(Type.String(), { description: "Regex domains to exclude" })),
      allow_external: Type.Optional(Type.Boolean({ description: "Allow external domains", default: false })),
      include_images: Type.Optional(Type.Boolean({ description: "Include image URLs", default: false })),
      include_favicon: Type.Optional(Type.Boolean({ description: "Include favicons", default: false })),
      format: Type.Optional(
        Type.String({
          description: "Content format, e.g. markdown or text",
          default: "markdown",
        }),
      ),
    }),
    async execute(_toolCallId, params) {
      const data = await tavilyPost("/crawl", {
        url: params.url,
        instructions: params.instructions,
        max_depth: params.max_depth ?? 1,
        max_breadth: params.max_breadth ?? 20,
        limit: params.limit ?? 20,
        select_paths: params.select_paths,
        exclude_paths: params.exclude_paths,
        select_domains: params.select_domains,
        exclude_domains: params.exclude_domains,
        allow_external: params.allow_external ?? false,
        include_images: params.include_images ?? false,
        include_favicon: params.include_favicon ?? false,
        format: params.format ?? "markdown",
      });

      return makeTextResult(data, {
        url: params.url,
        returned: Array.isArray(data?.results) ? data.results.length : undefined,
      });
    },
  });

  pi.registerTool({
    name: "tavily_map",
    label: "Tavily Map",
    description: "Map a site from a base URL and return discovered URLs without full page extraction.",
    parameters: Type.Object({
      url: Type.String({ description: "Base URL to map" }),
      instructions: Type.Optional(
        Type.String({
          description: "Natural-language instructions for link discovery",
        }),
      ),
      max_depth: Type.Optional(Type.Number({ description: "Maximum map depth", default: 1 })),
      max_breadth: Type.Optional(
        Type.Number({
          description: "Maximum links followed per level",
          default: 20,
        }),
      ),
      limit: Type.Optional(Type.Number({ description: "Total result limit", default: 50 })),
      select_paths: Type.Optional(
        Type.Array(Type.String(), {
          description: "Regex path patterns to include",
        }),
      ),
      exclude_paths: Type.Optional(
        Type.Array(Type.String(), {
          description: "Regex path patterns to exclude",
        }),
      ),
      select_domains: Type.Optional(Type.Array(Type.String(), { description: "Regex domains to include" })),
      exclude_domains: Type.Optional(Type.Array(Type.String(), { description: "Regex domains to exclude" })),
      allow_external: Type.Optional(Type.Boolean({ description: "Allow external domains", default: false })),
    }),
    async execute(_toolCallId, params) {
      const data = await tavilyPost("/map", {
        url: params.url,
        instructions: params.instructions,
        max_depth: params.max_depth ?? 1,
        max_breadth: params.max_breadth ?? 20,
        limit: params.limit ?? 50,
        select_paths: params.select_paths,
        exclude_paths: params.exclude_paths,
        select_domains: params.select_domains,
        exclude_domains: params.exclude_domains,
        allow_external: params.allow_external ?? false,
      });

      return makeTextResult(data, {
        url: params.url,
        returned: Array.isArray(data?.results) ? data.results.length : undefined,
      });
    },
  });
}
