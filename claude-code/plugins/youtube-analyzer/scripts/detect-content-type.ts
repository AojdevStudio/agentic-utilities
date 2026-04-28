#!/usr/bin/env bun

// DetectContentType.ts — Classifies YouTube videos by category + format
// Usage:
//   bun run DetectContentType.ts --url "https://youtube.com/watch?v=..."
//   echo '{"title":"...","description":"...","tags":[...]}' | bun run DetectContentType.ts --json
//
// Output: JSON { category, format, confidence, metadata }

import { parseArgs } from "node:util";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    url: { type: "string" },
    json: { type: "boolean", default: false },
  },
  strict: true,
});

interface VideoMetadata {
  title: string;
  description: string;
  tags?: string[];
  channel?: string;
  category_id?: string;
  duration?: number;
  upload_date?: string;
  view_count?: number;
}

interface ClassificationResult {
  category: string;
  format: string;
  confidence: number;
  metadata: {
    title: string;
    channel?: string;
    duration?: number;
    duration_formatted?: string;
    upload_date?: string;
    view_count?: number;
    description_preview: string;
    tags?: string[];
  };
  error?: string;
}

const CATEGORY_KEYWORDS: Record<string, { keywords: string[]; weight: number }[]> = {
  technology: [
    {
      keywords: [
        "programming",
        "coding",
        "developer",
        "software",
        "javascript",
        "typescript",
        "python",
        "react",
        "nextjs",
        "api",
        "web development",
        "frontend",
        "backend",
        "fullstack",
        "devops",
        "docker",
        "kubernetes",
        "git",
        "vscode",
        "npm",
      ],
      weight: 3,
    },
    {
      keywords: ["tech", "computer", "algorithm", "database", "framework", "library", "deploy", "debug", "refactor"],
      weight: 2,
    },
  ],
  finance: [
    {
      keywords: [
        "investing",
        "dividend",
        "portfolio",
        "stocks",
        "market",
        "trading",
        "etf",
        "yield",
        "margin",
        "options",
        "crypto",
        "bitcoin",
        "wealth",
        "passive income",
        "fire",
        "retirement",
      ],
      weight: 3,
    },
    {
      keywords: ["money", "finance", "economy", "inflation", "interest rate", "budget", "savings", "real estate"],
      weight: 2,
    },
  ],
  business: [
    {
      keywords: [
        "entrepreneur",
        "startup",
        "management",
        "leadership",
        "marketing",
        "sales",
        "revenue",
        "scaling",
        "dental practice",
        "consulting",
      ],
      weight: 3,
    },
    { keywords: ["business", "company", "strategy", "growth", "profit", "team", "hiring", "operations"], weight: 2 },
  ],
  health: [
    {
      keywords: [
        "fitness",
        "nutrition",
        "workout",
        "exercise",
        "diet",
        "mental health",
        "wellness",
        "meditation",
        "yoga",
        "supplement",
      ],
      weight: 3,
    },
    { keywords: ["health", "body", "weight", "sleep", "stress", "recovery", "training"], weight: 2 },
  ],
  education: [
    {
      keywords: [
        "course",
        "university",
        "academic",
        "learning",
        "teaching",
        "study",
        "exam",
        "degree",
        "professor",
        "lecture",
      ],
      weight: 3,
    },
    { keywords: ["education", "school", "student", "knowledge", "skill", "certification"], weight: 2 },
  ],
  science: [
    {
      keywords: [
        "research",
        "physics",
        "chemistry",
        "biology",
        "space",
        "nasa",
        "quantum",
        "evolution",
        "climate",
        "experiment",
      ],
      weight: 3,
    },
    { keywords: ["science", "scientific", "discovery", "theory", "data", "lab", "study"], weight: 2 },
  ],
  entertainment: [
    { keywords: ["gaming", "movie", "film", "music", "sports", "comedy", "review", "reaction", "vlog"], weight: 3 },
    { keywords: ["entertainment", "fun", "play", "watch", "stream", "content"], weight: 2 },
  ],
  politics: [
    {
      keywords: [
        "government",
        "policy",
        "election",
        "congress",
        "democrat",
        "republican",
        "legislation",
        "geopolitics",
        "tariff",
      ],
      weight: 3,
    },
    { keywords: ["politics", "political", "vote", "law", "regulation", "international"], weight: 2 },
  ],
  general: [{ keywords: [], weight: 1 }],
};

const FORMAT_KEYWORDS: Record<string, { keywords: string[]; weight: number }[]> = {
  tutorial: [
    {
      keywords: [
        "tutorial",
        "how to",
        "build",
        "code along",
        "step by step",
        "from scratch",
        "crash course",
        "beginner",
        "let's build",
        "project",
      ],
      weight: 3,
    },
    { keywords: ["walkthrough", "demo", "implementation", "setup", "install", "configure"], weight: 2 },
  ],
  course: [
    { keywords: ["full course", "complete course", "masterclass", "bootcamp", "comprehensive course"], weight: 3 },
  ],
  finance: [
    {
      keywords: [
        "investing strategy",
        "portfolio review",
        "dividend",
        "market analysis",
        "stock pick",
        "financial freedom",
      ],
      weight: 3,
    },
  ],
  sermon: [
    {
      keywords: [
        "sermon",
        "preaching",
        "scripture",
        "bible",
        "pastor",
        "church",
        "worship",
        "gospel",
        "testimony",
        "faith",
      ],
      weight: 3,
    },
    { keywords: ["pray", "god", "jesus", "christian", "ministry", "spiritual"], weight: 2 },
  ],
  interview: [
    {
      keywords: ["interview", "conversation", "podcast", "q&a", "talks with", "sits down with", "chats with"],
      weight: 3,
    },
  ],
  lecture: [{ keywords: ["lecture", "class", "seminar", "keynote", "talk", "presentation", "ted"], weight: 3 }],
  general: [{ keywords: [], weight: 1 }],
};

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }
  return `${minutes}:${String(secs).padStart(2, "0")}`;
}

function scoreText(text: string, keywords: { keywords: string[]; weight: number }[]): number {
  let score = 0;
  const lowerText = text.toLowerCase();

  for (const { keywords: keywordList, weight } of keywords) {
    for (const keyword of keywordList) {
      if (lowerText.includes(keyword.toLowerCase())) {
        score += weight;
      }
    }
  }

  return score;
}

function classifyContent(metadata: VideoMetadata): Omit<ClassificationResult, "metadata"> {
  const categoryScores: Record<string, number> = {};
  const formatScores: Record<string, number> = {};

  // Score categories
  for (const [category, keywords] of Object.entries(CATEGORY_KEYWORDS)) {
    let score = 0;
    // Title gets 3x multiplier
    score += scoreText(metadata.title, keywords) * 3;
    // Description gets 2x multiplier
    score += scoreText(metadata.description, keywords) * 2;
    // Tags get 1x multiplier
    if (metadata.tags) {
      score += scoreText(metadata.tags.join(" "), keywords);
    }
    categoryScores[category] = score;
  }

  // Score formats
  for (const [format, keywords] of Object.entries(FORMAT_KEYWORDS)) {
    let score = 0;
    score += scoreText(metadata.title, keywords) * 3;
    score += scoreText(metadata.description, keywords) * 2;
    if (metadata.tags) {
      score += scoreText(metadata.tags.join(" "), keywords);
    }
    formatScores[format] = score;
  }

  // Find highest scoring category
  let topCategory = "general";
  let topCategoryScore = 0;
  for (const [category, score] of Object.entries(categoryScores)) {
    if (score > topCategoryScore) {
      topCategoryScore = score;
      topCategory = category;
    }
  }

  // Find highest scoring format
  let topFormat = "general";
  let topFormatScore = 0;
  for (const [format, score] of Object.entries(formatScores)) {
    if (score > topFormatScore) {
      topFormatScore = score;
      topFormat = format;
    }
  }

  // Upgrade tutorial to course if duration > 3 hours
  if (topFormat === "tutorial" && metadata.duration && metadata.duration > 10800) {
    topFormat = "course";
  }

  // Calculate confidence (max possible score from title + description + tags)
  const maxPossibleCategoryScore =
    CATEGORY_KEYWORDS[topCategory].reduce((sum, item) => sum + item.keywords.length * item.weight, 0) * 6; // 3x title + 2x desc + 1x tags
  const maxPossibleFormatScore =
    FORMAT_KEYWORDS[topFormat].reduce((sum, item) => sum + item.keywords.length * item.weight, 0) * 6;

  const categoryConfidence = maxPossibleCategoryScore > 0 ? (topCategoryScore / maxPossibleCategoryScore) * 100 : 0;
  const formatConfidence = maxPossibleFormatScore > 0 ? (topFormatScore / maxPossibleFormatScore) * 100 : 0;

  // Average the two confidences
  const confidence = Math.min(99, Math.round((categoryConfidence + formatConfidence) / 2));

  return {
    category: topCategory,
    format: topFormat,
    confidence,
  };
}

async function fetchVideoMetadata(url: string): Promise<VideoMetadata> {
  const proc = Bun.spawn(["yt-dlp", "--dump-json", "--skip-download", url], {
    stdout: "pipe",
    stderr: "pipe",
  });

  const output = await new Response(proc.stdout).text();
  const exitCode = await proc.exited;

  if (exitCode !== 0) {
    const error = await new Response(proc.stderr).text();
    throw new Error(`yt-dlp failed: ${error}`);
  }

  const data = JSON.parse(output);

  return {
    title: data.title || "",
    description: data.description || "",
    tags: data.tags || [],
    channel: data.channel || data.uploader,
    category_id: data.category_id,
    duration: data.duration,
    upload_date: data.upload_date,
    view_count: data.view_count,
  };
}

async function main() {
  try {
    let metadata: VideoMetadata;

    if (values.url) {
      metadata = await fetchVideoMetadata(values.url);
    } else if (values.json) {
      const stdinText = await Bun.stdin.text();
      metadata = JSON.parse(stdinText);
    } else {
      console.error("Error: Must provide either --url or --json flag");
      process.exit(1);
    }

    const classification = classifyContent(metadata);

    const result: ClassificationResult = {
      ...classification,
      metadata: {
        title: metadata.title,
        channel: metadata.channel,
        duration: metadata.duration,
        duration_formatted: metadata.duration ? formatDuration(metadata.duration) : undefined,
        upload_date: metadata.upload_date,
        view_count: metadata.view_count,
        description_preview: metadata.description.substring(0, 200),
        tags: metadata.tags,
      },
    };

    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const result: ClassificationResult = {
      category: "general",
      format: "general",
      confidence: 0,
      metadata: {
        title: "",
        description_preview: "",
      },
      error: error instanceof Error ? error.message : String(error),
    };
    console.log(JSON.stringify(result, null, 2));
    process.exit(1);
  }
}

main();
