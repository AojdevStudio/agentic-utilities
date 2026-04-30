#!/usr/bin/env bun

// CleanTranscript.ts — Strips VTT artifacts from repo-sourced transcripts
// Usage:
//   bun run CleanTranscript.ts --input "/path/to/raw-transcript.md" --output-dir "/tmp/scratchpad"
//
// Output: JSON { cleanPath, wordCount, lineCount, metadata }

import { existsSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    input: { type: "string" },
    "output-dir": { type: "string" },
  },
  strict: true,
});

interface CleanResult {
  cleanPath: string;
  wordCount: number;
  lineCount: number;
  metadata: Record<string, string>;
  error?: string;
}

function parseFrontmatter(text: string): { metadata: Record<string, string>; body: string } {
  const metadata: Record<string, string> = {};

  if (!text.startsWith("---")) {
    return { metadata, body: text };
  }

  const endIndex = text.indexOf("\n---", 3);
  if (endIndex === -1) {
    return { metadata, body: text };
  }

  const frontmatterBlock = text.substring(3, endIndex).trim();
  const body = text.substring(endIndex + 4).trim();

  for (const line of frontmatterBlock.split("\n")) {
    const colonIndex = line.indexOf(":");
    if (colonIndex === -1) continue;

    const key = line.substring(0, colonIndex).trim();
    let value = line.substring(colonIndex + 1).trim();

    // Strip surrounding quotes
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1);
    }

    metadata[key] = value;
  }

  return { metadata, body };
}

function cleanVttArtifacts(body: string): string {
  const lines = body.split("\n");
  const cleaned: string[] = [];

  for (const line of lines) {
    // Rule 1: Remove VTT header lines
    if (/^Kind:\s*captions/i.test(line)) continue;
    if (/^Language:\s*\w+/i.test(line)) continue;

    // Rule 2: Remove inline timestamp lines
    // Lines containing <c> tags (VTT cue payload tags)
    if (/<c>/.test(line)) continue;
    // Lines containing <00:00: timestamp patterns
    if (/<\d{2}:\d{2}:\d{2}/.test(line)) continue;
    // Standalone VTT timestamp lines (e.g., "00:00:01.234 --> 00:00:05.678")
    if (/^\d{2}:\d{2}:\d{2}\.\d{3}\s*-->/.test(line)) continue;
    // Standalone numeric cue identifiers
    if (/^\d+$/.test(line.trim()) && line.trim().length <= 6) continue;

    cleaned.push(line);
  }

  // Rule 3: Deduplicate consecutive identical lines
  const deduped: string[] = [];
  for (const line of cleaned) {
    if (deduped.length === 0 || line !== deduped[deduped.length - 1]) {
      deduped.push(line);
    }
  }

  // Rule 4: Collapse 3+ consecutive blank lines to single blank
  const collapsed: string[] = [];
  let consecutiveBlanks = 0;

  for (const line of deduped) {
    if (line.trim() === "") {
      consecutiveBlanks++;
      if (consecutiveBlanks <= 1) {
        collapsed.push(line);
      }
    } else {
      consecutiveBlanks = 0;
      collapsed.push(line);
    }
  }

  return collapsed.join("\n").trim();
}

async function main() {
  try {
    if (!values.input) {
      console.error("Error: --input is required");
      process.exit(1);
    }

    const inputPath = values.input;

    if (!existsSync(inputPath)) {
      const result: CleanResult = {
        cleanPath: "",
        wordCount: 0,
        lineCount: 0,
        metadata: {},
        error: `Input file does not exist: ${inputPath}`,
      };
      console.log(JSON.stringify(result, null, 2));
      process.exit(1);
    }

    const rawText = await Bun.file(inputPath).text();

    // Rule 5: Preserve YAML frontmatter and parse for metadata
    const { metadata, body } = parseFrontmatter(rawText);

    // Apply cleaning rules to the body
    const cleanedBody = cleanVttArtifacts(body);

    // Calculate stats
    const lines = cleanedBody.split("\n");
    const lineCount = lines.length;
    const wordCount = cleanedBody.split(/\s+/).filter((w) => w.length > 0).length;

    // Determine output path
    const outputDir = values["output-dir"] || path.dirname(inputPath);
    const basename = path.basename(inputPath, path.extname(inputPath));
    const cleanPath = path.join(outputDir, `${basename}-clean.txt`);

    // Write clean text (body only, no frontmatter)
    await Bun.write(cleanPath, cleanedBody);

    const result: CleanResult = {
      cleanPath,
      wordCount,
      lineCount,
      metadata,
    };

    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const result: CleanResult = {
      cleanPath: "",
      wordCount: 0,
      lineCount: 0,
      metadata: {},
      error: error instanceof Error ? error.message : String(error),
    };
    console.log(JSON.stringify(result, null, 2));
    process.exit(1);
  }
}

main();
