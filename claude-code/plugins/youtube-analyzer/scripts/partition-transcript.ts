#!/usr/bin/env bun

// PartitionTranscript.ts — Splits transcript for parallel sub-agent analysis
// Usage:
//   bun run PartitionTranscript.ts --input transcript.txt --max-lines-per-agent 5000
//   bun run PartitionTranscript.ts --input transcript.txt --max-lines-per-agent 5000 --output-dir /tmp/chunks/
//
// Output: JSON array of chunk definitions

import { existsSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    input: { type: "string" },
    "max-lines-per-agent": { type: "string", default: "5000" },
    "output-dir": { type: "string" },
  },
  strict: true,
});

interface ChunkInfo {
  chunk: number;
  startLine: number;
  endLine: number;
  lineCount: number;
  file: string;
}

interface PartitionResult {
  totalLines: number;
  totalChunks: number;
  maxLinesPerAgent: number;
  partitioned: boolean;
  chunks: ChunkInfo[];
  error?: string;
}

async function countLines(filePath: string): Promise<number> {
  const file = Bun.file(filePath);
  const text = await file.text();
  return text.split("\n").length;
}

async function readLines(filePath: string, startLine: number, endLine: number): Promise<string> {
  const file = Bun.file(filePath);
  const text = await file.text();
  const lines = text.split("\n");
  // startLine is 1-indexed, convert to 0-indexed
  return lines.slice(startLine - 1, endLine).join("\n");
}

async function writeChunk(content: string, outputPath: string): Promise<void> {
  await Bun.write(outputPath, content);
}

async function main() {
  try {
    if (!values.input) {
      console.error("Error: --input is required");
      process.exit(1);
    }

    const inputPath = values.input;

    if (!existsSync(inputPath)) {
      const result: PartitionResult = {
        totalLines: 0,
        totalChunks: 0,
        maxLinesPerAgent: parseInt(values["max-lines-per-agent"], 10),
        partitioned: false,
        chunks: [],
        error: `Input file does not exist: ${inputPath}`,
      };
      console.log(JSON.stringify(result, null, 2));
      process.exit(1);
      return;
    }

    const maxLinesPerAgent = parseInt(values["max-lines-per-agent"], 10);
    const totalLines = await countLines(inputPath);

    // Determine output directory
    const outputDir = values["output-dir"] || path.dirname(inputPath);
    const basename = path.basename(inputPath, path.extname(inputPath));

    if (totalLines <= maxLinesPerAgent) {
      // No partition needed
      const result: PartitionResult = {
        totalLines,
        totalChunks: 1,
        maxLinesPerAgent,
        partitioned: false,
        chunks: [
          {
            chunk: 1,
            startLine: 1,
            endLine: totalLines,
            lineCount: totalLines,
            file: inputPath,
          },
        ],
      };
      console.log(JSON.stringify(result, null, 2));
      return;
    }

    // Need to partition
    const totalChunks = Math.ceil(totalLines / maxLinesPerAgent);
    const chunks: ChunkInfo[] = [];

    for (let i = 0; i < totalChunks; i++) {
      const chunkNumber = i + 1;
      const startLine = i * maxLinesPerAgent + 1;
      const endLine = Math.min((i + 1) * maxLinesPerAgent, totalLines);
      const lineCount = endLine - startLine + 1;

      const chunkFileName = `${basename}-chunk-${chunkNumber}.txt`;
      const chunkFilePath = path.join(outputDir, chunkFileName);

      // Read the chunk content
      const chunkContent = await readLines(inputPath, startLine, endLine);

      // Write the chunk file
      await writeChunk(chunkContent, chunkFilePath);

      chunks.push({
        chunk: chunkNumber,
        startLine,
        endLine,
        lineCount,
        file: chunkFilePath,
      });
    }

    const result: PartitionResult = {
      totalLines,
      totalChunks,
      maxLinesPerAgent,
      partitioned: true,
      chunks,
    };

    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const result: PartitionResult = {
      totalLines: 0,
      totalChunks: 0,
      maxLinesPerAgent: parseInt(values["max-lines-per-agent"], 10),
      partitioned: false,
      chunks: [],
      error: error instanceof Error ? error.message : String(error),
    };
    console.log(JSON.stringify(result, null, 2));
    process.exit(1);
  }
}

main();
