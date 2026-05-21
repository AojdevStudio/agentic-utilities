#!/usr/bin/env bun

/**
 * index-tree.ts -- RepoArchitect Working-Tree Indexer
 *
 * Scans a git working tree, classifies every file, and produces three JSON
 * artefacts that tell you what is tracked, what should be tracked, and what
 * should be ignored.
 *
 * Usage:
 *   bun run ${CLAUDE_PLUGIN_ROOT}/skills/repo-architect/tools/index-tree.ts [options] [repo-path]
 *
 * Flags:
 *   --diff              Compare against previous repo.index.json
 *   --top-untracked N   Print N highest-priority untracked candidates
 *   --apply-ignore      Append suggested patterns to .gitignore
 *   --output-dir DIR    Where to write output (default: .dev/)
 *   --quiet             Suppress progress output
 */

import { existsSync, mkdirSync, statSync } from "node:fs";
import { basename, extname, join, resolve } from "node:path";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type FileCategory = "source" | "config" | "docs" | "test" | "script" | "asset" | "generated" | "unknown";

type Priority = "high" | "medium" | "low";

interface FileEntry {
  path: string;
  tracked: boolean;
  size: number;
  mtime: string;
  category: FileCategory;
}

interface RepoIndex {
  generated_at: string;
  repo_path: string;
  total_files: number;
  tracked: number;
  untracked: number;
  files: FileEntry[];
}

interface TrackSuggestion {
  path: string;
  reason: string;
  size: number;
  priority: Priority;
}

interface TrackSuggestions {
  generated_at: string;
  suggestions: TrackSuggestion[];
}

interface IgnoreSuggestion {
  pattern: string;
  reason: string;
  count: number;
  total_size: number;
}

interface IgnoreSuggestions {
  generated_at: string;
  suggestions: IgnoreSuggestion[];
}

interface DiffResult {
  added: string[];
  removed: string[];
  changed: string[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SOURCE_DIRS = new Set([
  "src",
  "app",
  "packages",
  "infra",
  "scripts",
  "docs",
  "lib",
  "tools",
  ".claude",
  ".github",
  ".dev",
]);

const CODE_EXTENSIONS = new Set([
  ".ts",
  ".tsx",
  ".js",
  ".jsx",
  ".py",
  ".sql",
  ".md",
  ".yaml",
  ".yml",
  ".json",
  ".toml",
  ".sh",
  ".css",
  ".html",
  ".go",
  ".rs",
  ".env.example",
]);

const BINARY_EXTENSIONS = new Set([
  ".png",
  ".jpg",
  ".jpeg",
  ".gif",
  ".webp",
  ".ico",
  ".svg",
  ".mp4",
  ".mp3",
  ".wav",
  ".ogg",
  ".woff",
  ".woff2",
  ".ttf",
  ".eot",
  ".otf",
  ".zip",
  ".tar",
  ".gz",
  ".bz2",
  ".7z",
  ".rar",
  ".pdf",
  ".exe",
  ".dll",
  ".so",
  ".dylib",
  ".o",
  ".a",
  ".wasm",
]);

const BUILD_DIRS = new Set(["dist", ".next", "build", "coverage", "out", "__pycache__"]);

const CACHE_DIRS = new Set([".cache", ".turbo", ".parcel-cache"]);

const LOG_EXTENSIONS = new Set([".log", ".tmp", ".swp"]);

const OS_FILES = new Set([".DS_Store", "Thumbs.db"]);

const CONFIG_EXTENSIONS = new Set([".json", ".yaml", ".yml", ".toml", ".ini", ".cfg", ".conf", ".env.example"]);

const DOC_EXTENSIONS = new Set([".md", ".mdx", ".txt", ".rst", ".adoc"]);

const SCRIPT_EXTENSIONS = new Set([".sh", ".bash", ".zsh", ".fish", ".ps1"]);

const TEST_PATTERNS = [/\.test\./, /\.spec\./, /__tests__\//, /tests?\//, /\.stories\./];

const ASSET_EXTENSIONS = new Set([
  ".png",
  ".jpg",
  ".jpeg",
  ".gif",
  ".webp",
  ".ico",
  ".svg",
  ".mp4",
  ".mp3",
  ".wav",
  ".pdf",
  ".woff",
  ".woff2",
  ".ttf",
  ".eot",
  ".otf",
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function log(quiet: boolean, ...args: unknown[]): void {
  if (!quiet) {
    console.error("[index-tree]", ...args);
  }
}

async function spawn(cmd: string[], cwd: string): Promise<{ stdout: string; exitCode: number }> {
  const proc = Bun.spawn(cmd, {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = await new Response(proc.stdout).text();
  const exitCode = await proc.exited;
  return { stdout, exitCode };
}

// ---------------------------------------------------------------------------
// Step 1: List all files via ripgrep (fast, .gitignore-aware)
// ---------------------------------------------------------------------------

async function listAllFiles(repoPath: string): Promise<string[]> {
  const { stdout, exitCode } = await spawn(["rg", "--files", "--hidden", "--glob", "!.git/*"], repoPath);

  if (exitCode !== 0 && exitCode !== 1) {
    // exitCode 1 means no matches, which is valid for empty repos
    // Check if rg exists
    const which = await spawn(["which", "rg"], repoPath);
    if (which.exitCode !== 0) {
      console.error("Error: ripgrep (rg) is required. Install with: brew install ripgrep");
      process.exit(1);
    }
    console.error(`Error: rg exited with code ${exitCode}`);
    process.exit(1);
  }

  return stdout
    .trim()
    .split("\n")
    .filter((line) => line.length > 0);
}

// ---------------------------------------------------------------------------
// Step 2: Get tracked files via git ls-files
// ---------------------------------------------------------------------------

async function getTrackedFiles(repoPath: string): Promise<Set<string>> {
  const { stdout, exitCode } = await spawn(["git", "ls-files", "-z"], repoPath);

  if (exitCode !== 0) {
    console.error("Error: Not a git repository. Run from a git repo root.");
    process.exit(1);
  }

  const files = stdout.split("\0").filter((f) => f.length > 0);

  return new Set(files);
}

// ---------------------------------------------------------------------------
// Step 3: Collect metadata (stat only, no content reading)
// ---------------------------------------------------------------------------

interface FileStat {
  size: number;
  mtime: Date;
  mode: number;
}

async function statFiles(repoPath: string, filePaths: string[], quiet: boolean): Promise<Map<string, FileStat>> {
  const results = new Map<string, FileStat>();
  const BATCH_SIZE = 200;

  for (let i = 0; i < filePaths.length; i += BATCH_SIZE) {
    const batch = filePaths.slice(i, i + BATCH_SIZE);
    const promises = batch.map(async (fp) => {
      try {
        const fullPath = join(repoPath, fp);
        const st = statSync(fullPath);
        results.set(fp, {
          size: st.size,
          mtime: st.mtime,
          mode: st.mode,
        });
      } catch {
        // File may have been deleted between listing and stat
      }
    });
    await Promise.all(promises);

    if ((i + BATCH_SIZE) % 2000 === 0) {
      log(quiet, `  stat progress: ${Math.min(i + BATCH_SIZE, filePaths.length)}/${filePaths.length}`);
    }
  }

  return results;
}

// ---------------------------------------------------------------------------
// Step 4: Classification heuristics
// ---------------------------------------------------------------------------

function classifyCategory(filePath: string): FileCategory {
  const ext = extname(filePath).toLowerCase();
  const base = basename(filePath);
  const parts = filePath.split("/");

  // Test files
  if (TEST_PATTERNS.some((p) => p.test(filePath))) {
    return "test";
  }

  // Generated / build output
  if (parts.some((p) => BUILD_DIRS.has(p) || CACHE_DIRS.has(p))) {
    return "generated";
  }
  if (parts.includes("node_modules")) {
    return "generated";
  }

  // Scripts
  if (SCRIPT_EXTENSIONS.has(ext)) {
    return "script";
  }

  // Docs
  if (DOC_EXTENSIONS.has(ext)) {
    return "docs";
  }

  // Assets
  if (ASSET_EXTENSIONS.has(ext)) {
    return "asset";
  }

  // Config files at root level or config extensions
  const configNames = new Set([
    "package.json",
    "tsconfig.json",
    "biome.json",
    "eslint.config.js",
    ".eslintrc",
    ".prettierrc",
    "jest.config.ts",
    "vitest.config.ts",
    "bun.lockb",
    "Makefile",
    "Justfile",
    "justfile",
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    ".gitignore",
    ".gitattributes",
    ".editorconfig",
    ".nvmrc",
    ".node-version",
    ".tool-versions",
  ]);

  if (configNames.has(base)) {
    return "config";
  }

  // Config by extension when not in source dirs
  if (CONFIG_EXTENSIONS.has(ext) && !parts.some((p) => SOURCE_DIRS.has(p))) {
    return "config";
  }

  // Source code
  const sourceExts = new Set([
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".py",
    ".go",
    ".rs",
    ".sql",
    ".css",
    ".html",
    ".vue",
    ".svelte",
    ".astro",
  ]);
  if (sourceExts.has(ext)) {
    return "source";
  }

  return "unknown";
}

function isInSourceDir(filePath: string): boolean {
  const parts = filePath.split("/");
  return parts.some((p) => SOURCE_DIRS.has(p));
}

function isCodeExtension(filePath: string): boolean {
  const ext = extname(filePath).toLowerCase();
  // Handle compound extensions like .env.example
  if (filePath.endsWith(".env.example")) return true;
  return CODE_EXTENSIONS.has(ext);
}

function shouldBeIgnored(filePath: string, size: number): { ignore: boolean; reason: string } {
  const ext = extname(filePath).toLowerCase();
  const base = basename(filePath);
  const parts = filePath.split("/");

  // OS files
  if (OS_FILES.has(base)) {
    return { ignore: true, reason: "OS metadata file" };
  }

  // Log/temp files
  if (LOG_EXTENSIONS.has(ext)) {
    return { ignore: true, reason: "Temporary/log file" };
  }

  // .env (but not .env.example)
  if (base === ".env" || (base.startsWith(".env.") && !base.endsWith(".example"))) {
    return { ignore: true, reason: "Environment secrets file" };
  }

  // Build directories
  if (parts.some((p) => BUILD_DIRS.has(p))) {
    return { ignore: true, reason: `Build directory (${parts.find((p) => BUILD_DIRS.has(p))})` };
  }

  // Cache directories
  if (parts.some((p) => CACHE_DIRS.has(p))) {
    return { ignore: true, reason: `Cache directory (${parts.find((p) => CACHE_DIRS.has(p))})` };
  }

  // node_modules
  if (parts.includes("node_modules")) {
    return { ignore: true, reason: "node_modules directory" };
  }

  // Large binary artefacts (>1MB with binary extension)
  if (size > 1_000_000 && BINARY_EXTENSIONS.has(ext)) {
    return { ignore: true, reason: `Large binary file (${(size / 1_000_000).toFixed(1)}MB)` };
  }

  return { ignore: false, reason: "" };
}

function computeTrackPriority(filePath: string): Priority {
  const ext = extname(filePath).toLowerCase();

  // High: code in standard source dirs
  if (isInSourceDir(filePath) && isCodeExtension(filePath)) {
    return "high";
  }

  // High: code files even outside standard dirs
  const highExts = new Set([".ts", ".tsx", ".js", ".jsx", ".go", ".rs", ".py"]);
  if (highExts.has(ext)) {
    return "high";
  }

  // Medium: config/docs
  if (CONFIG_EXTENSIONS.has(ext) || DOC_EXTENSIONS.has(ext) || SCRIPT_EXTENSIONS.has(ext)) {
    return "medium";
  }

  return "low";
}

function computeTrackReason(filePath: string): string {
  const ext = extname(filePath).toLowerCase();
  const parts = filePath.split("/");

  if (isInSourceDir(filePath)) {
    const dir = parts.find((p) => SOURCE_DIRS.has(p));
    return `Source file in ${dir}/ directory`;
  }

  if (isCodeExtension(filePath)) {
    return `Code file (${ext})`;
  }

  if (DOC_EXTENSIONS.has(ext)) {
    return `Documentation file (${ext})`;
  }

  if (CONFIG_EXTENSIONS.has(ext)) {
    return `Configuration file (${ext})`;
  }

  if (SCRIPT_EXTENSIONS.has(ext)) {
    return `Script file (${ext})`;
  }

  return "Untracked file";
}

// ---------------------------------------------------------------------------
// Step 5: Output generation
// ---------------------------------------------------------------------------

function buildRepoIndex(
  repoPath: string,
  allFiles: string[],
  trackedSet: Set<string>,
  statMap: Map<string, FileStat>,
): RepoIndex {
  const files: FileEntry[] = [];

  for (const fp of allFiles) {
    const st = statMap.get(fp);
    if (!st) continue;

    files.push({
      path: fp,
      tracked: trackedSet.has(fp),
      size: st.size,
      mtime: st.mtime.toISOString(),
      category: classifyCategory(fp),
    });
  }

  const tracked = files.filter((f) => f.tracked).length;

  return {
    generated_at: new Date().toISOString(),
    repo_path: repoPath,
    total_files: files.length,
    tracked,
    untracked: files.length - tracked,
    files,
  };
}

function buildTrackSuggestions(
  allFiles: string[],
  trackedSet: Set<string>,
  statMap: Map<string, FileStat>,
): TrackSuggestions {
  const suggestions: TrackSuggestion[] = [];

  for (const fp of allFiles) {
    if (trackedSet.has(fp)) continue;

    const st = statMap.get(fp);
    if (!st) continue;

    const { ignore } = shouldBeIgnored(fp, st.size);
    if (ignore) continue;

    const priority = computeTrackPriority(fp);
    const reason = computeTrackReason(fp);

    // Only suggest files that have some signal of being trackable
    if (isCodeExtension(fp) || isInSourceDir(fp) || priority !== "low") {
      suggestions.push({
        path: fp,
        reason,
        size: st.size,
        priority,
      });
    }
  }

  // Sort: high > medium > low, then by path
  const priorityOrder: Record<Priority, number> = {
    high: 0,
    medium: 1,
    low: 2,
  };
  suggestions.sort((a, b) => {
    const pd = priorityOrder[a.priority] - priorityOrder[b.priority];
    if (pd !== 0) return pd;
    return a.path.localeCompare(b.path);
  });

  return {
    generated_at: new Date().toISOString(),
    suggestions,
  };
}

function buildIgnoreSuggestions(
  allFiles: string[],
  trackedSet: Set<string>,
  statMap: Map<string, FileStat>,
): IgnoreSuggestions {
  // Group by ignore pattern
  const patternMap = new Map<string, { reason: string; count: number; total_size: number }>();

  for (const fp of allFiles) {
    if (trackedSet.has(fp)) continue;

    const st = statMap.get(fp);
    if (!st) continue;

    const { ignore, reason } = shouldBeIgnored(fp, st.size);
    if (!ignore) continue;

    // Determine the pattern to suggest
    const pattern = deriveIgnorePattern(fp);
    const existing = patternMap.get(pattern);
    if (existing) {
      existing.count++;
      existing.total_size += st.size;
    } else {
      patternMap.set(pattern, {
        reason,
        count: 1,
        total_size: st.size,
      });
    }
  }

  const suggestions: IgnoreSuggestion[] = [];
  for (const [pattern, data] of patternMap) {
    suggestions.push({
      pattern,
      reason: data.reason,
      count: data.count,
      total_size: data.total_size,
    });
  }

  // Sort by total size descending
  suggestions.sort((a, b) => b.total_size - a.total_size);

  return {
    generated_at: new Date().toISOString(),
    suggestions,
  };
}

function deriveIgnorePattern(filePath: string): string {
  const ext = extname(filePath).toLowerCase();
  const base = basename(filePath);
  const parts = filePath.split("/");

  // OS files -- exact name
  if (OS_FILES.has(base)) return base;

  // .env variants
  if (base === ".env") return ".env";
  if (base.startsWith(".env.") && !base.endsWith(".example")) return ".env.*";

  // Build directories
  for (const d of BUILD_DIRS) {
    if (parts.includes(d)) return `${d}/`;
  }

  // Cache directories
  for (const d of CACHE_DIRS) {
    if (parts.includes(d)) return `${d}/`;
  }

  // node_modules
  if (parts.includes("node_modules")) return "node_modules/";

  // Extension-based
  if (LOG_EXTENSIONS.has(ext)) return `*${ext}`;

  // Large binary
  if (BINARY_EXTENSIONS.has(ext)) return `*${ext}`;

  // Fallback: use the exact relative path
  return filePath;
}

// ---------------------------------------------------------------------------
// --diff mode
// ---------------------------------------------------------------------------

async function runDiff(outputDir: string, currentIndex: RepoIndex): Promise<void> {
  const prevPath = join(outputDir, "repo.index.json");

  if (!existsSync(prevPath)) {
    console.log("No previous repo.index.json found. Nothing to diff against.");
    return;
  }

  const prevData = (await Bun.file(prevPath).json()) as RepoIndex;
  const prevMap = new Map(prevData.files.map((f) => [f.path, f]));
  const currMap = new Map(currentIndex.files.map((f) => [f.path, f]));

  const result: DiffResult = { added: [], removed: [], changed: [] };

  // Added: in current but not in previous
  for (const fp of currMap.keys()) {
    if (!prevMap.has(fp)) {
      result.added.push(fp);
    }
  }

  // Removed: in previous but not in current
  for (const fp of prevMap.keys()) {
    if (!currMap.has(fp)) {
      result.removed.push(fp);
    }
  }

  // Changed: different size or mtime
  for (const [fp, curr] of currMap) {
    const prev = prevMap.get(fp);
    if (prev && (prev.size !== curr.size || prev.mtime !== curr.mtime)) {
      result.changed.push(fp);
    }
  }

  // Print summary
  console.log("\n--- Diff Summary ---\n");

  if (result.added.length === 0 && result.removed.length === 0 && result.changed.length === 0) {
    console.log("No changes detected since last run.");
    return;
  }

  if (result.added.length > 0) {
    console.log(`  + ${result.added.length} new file(s)`);
    for (const fp of result.added.slice(0, 20)) {
      console.log(`    + ${fp}`);
    }
    if (result.added.length > 20) {
      console.log(`    ... and ${result.added.length - 20} more`);
    }
  }

  if (result.removed.length > 0) {
    console.log(`  - ${result.removed.length} removed file(s)`);
    for (const fp of result.removed.slice(0, 20)) {
      console.log(`    - ${fp}`);
    }
    if (result.removed.length > 20) {
      console.log(`    ... and ${result.removed.length - 20} more`);
    }
  }

  if (result.changed.length > 0) {
    console.log(`  ~ ${result.changed.length} changed file(s)`);
    for (const fp of result.changed.slice(0, 20)) {
      console.log(`    ~ ${fp}`);
    }
    if (result.changed.length > 20) {
      console.log(`    ... and ${result.changed.length - 20} more`);
    }
  }

  console.log("");
}

// ---------------------------------------------------------------------------
// --top-untracked N mode
// ---------------------------------------------------------------------------

function printTopUntracked(suggestions: TrackSuggestions, n: number, statMap: Map<string, FileStat>): void {
  // Sort by priority then by recency (newest mtime first)
  const priorityOrder: Record<Priority, number> = {
    high: 0,
    medium: 1,
    low: 2,
  };

  const sorted = [...suggestions.suggestions].sort((a, b) => {
    const pd = priorityOrder[a.priority] - priorityOrder[b.priority];
    if (pd !== 0) return pd;

    const aStat = statMap.get(a.path);
    const bStat = statMap.get(b.path);
    if (aStat && bStat) {
      return bStat.mtime.getTime() - aStat.mtime.getTime();
    }
    return 0;
  });

  const top = sorted.slice(0, n);

  if (top.length === 0) {
    console.log("\nNo untracked candidates found.\n");
    return;
  }

  console.log(`\n--- Top ${Math.min(n, top.length)} Untracked Candidates ---\n`);

  // Table header
  const pathWidth = Math.max(40, ...top.map((s) => s.path.length)) + 2;
  const header = ["Priority".padEnd(10), "Path".padEnd(pathWidth), "Size".padStart(10), "Reason"].join("  ");

  console.log(header);
  console.log("-".repeat(header.length));

  for (const s of top) {
    const sizeStr = formatSize(s.size);
    const row = [s.priority.padEnd(10), s.path.padEnd(pathWidth), sizeStr.padStart(10), s.reason].join("  ");
    console.log(row);
  }

  console.log("");
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

// ---------------------------------------------------------------------------
// --apply-ignore mode
// ---------------------------------------------------------------------------

async function applyIgnore(repoPath: string, ignoreSuggestions: IgnoreSuggestions): Promise<void> {
  if (ignoreSuggestions.suggestions.length === 0) {
    console.log("No ignore suggestions to apply.");
    return;
  }

  const gitignorePath = join(repoPath, ".gitignore");
  let existing = "";

  if (existsSync(gitignorePath)) {
    existing = await Bun.file(gitignorePath).text();
  }

  // Parse existing patterns to avoid duplicates
  const existingPatterns = new Set(
    existing
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0 && !line.startsWith("#")),
  );

  const newPatterns = ignoreSuggestions.suggestions.map((s) => s.pattern).filter((p) => !existingPatterns.has(p));

  if (newPatterns.length === 0) {
    console.log("All suggested patterns already exist in .gitignore.");
    return;
  }

  const today = new Date().toISOString().split("T")[0];
  const block = ["", `# Added by RepoArchitect indexer (${today})`, ...newPatterns, ""].join("\n");

  const finalContent = existing.endsWith("\n") ? existing + block : existing + "\n" + block;

  await Bun.write(gitignorePath, finalContent);

  console.log(`Added ${newPatterns.length} pattern(s) to .gitignore:`);
  for (const p of newPatterns) {
    console.log(`  + ${p}`);
  }
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

interface CliArgs {
  repoPath: string;
  diff: boolean;
  topUntracked: number | null;
  applyIgnore: boolean;
  outputDir: string | null;
  quiet: boolean;
}

function parseArgs(args: string[]): CliArgs {
  const result: CliArgs = {
    repoPath: process.cwd(),
    diff: false,
    topUntracked: null,
    applyIgnore: false,
    outputDir: null,
    quiet: false,
  };

  let i = 0;
  while (i < args.length) {
    const arg = args[i];

    if (arg === "--diff") {
      result.diff = true;
    } else if (arg === "--top-untracked") {
      i++;
      const n = parseInt(args[i], 10);
      if (Number.isNaN(n) || n <= 0) {
        console.error("Error: --top-untracked requires a positive integer.");
        process.exit(1);
      }
      result.topUntracked = n;
    } else if (arg === "--apply-ignore") {
      result.applyIgnore = true;
    } else if (arg === "--output-dir") {
      i++;
      if (!args[i]) {
        console.error("Error: --output-dir requires a path.");
        process.exit(1);
      }
      result.outputDir = args[i];
    } else if (arg === "--quiet") {
      result.quiet = true;
    } else if (arg.startsWith("-")) {
      console.error(`Error: Unknown flag: ${arg}`);
      console.error(
        "Usage: bun run index-tree.ts [--diff] [--top-untracked N] [--apply-ignore] [--output-dir DIR] [--quiet] [repo-path]",
      );
      process.exit(1);
    } else {
      // Positional argument: repo path
      result.repoPath = resolve(arg);
    }

    i++;
  }

  return result;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const args = parseArgs(Bun.argv.slice(2));
  const repoPath = resolve(args.repoPath);
  const outputDir = args.outputDir ? resolve(args.outputDir) : join(repoPath, ".dev");
  const quiet = args.quiet;

  // Verify the path exists
  if (!existsSync(repoPath)) {
    console.error(`Error: Path does not exist: ${repoPath}`);
    process.exit(1);
  }

  // Verify it is a git repo
  const gitDir = join(repoPath, ".git");
  if (!existsSync(gitDir)) {
    console.error("Error: Not a git repository. Run from a git repo root.");
    process.exit(1);
  }

  const startTime = performance.now();

  // Step 1: List all files
  log(quiet, "Step 1/5: Listing files via ripgrep...");
  const allFiles = await listAllFiles(repoPath);
  log(quiet, `  Found ${allFiles.length} files`);

  // Step 2: Get tracked files
  log(quiet, "Step 2/5: Getting tracked files from git...");
  const trackedSet = await getTrackedFiles(repoPath);
  log(quiet, `  ${trackedSet.size} tracked files`);

  // Step 3: Stat all files
  log(quiet, "Step 3/5: Collecting file metadata...");
  const statMap = await statFiles(repoPath, allFiles, quiet);
  log(quiet, `  Stat'd ${statMap.size} files`);

  // Step 4 + 5: Build outputs
  log(quiet, "Step 4/5: Classifying files...");
  const repoIndex = buildRepoIndex(repoPath, allFiles, trackedSet, statMap);
  const trackSuggestions = buildTrackSuggestions(allFiles, trackedSet, statMap);
  const ignoreSuggestions = buildIgnoreSuggestions(allFiles, trackedSet, statMap);
  log(
    quiet,
    `  ${trackSuggestions.suggestions.length} track suggestions, ${ignoreSuggestions.suggestions.length} ignore suggestions`,
  );

  // Handle --diff BEFORE writing (compare against existing file)
  if (args.diff) {
    await runDiff(outputDir, repoIndex);
  }

  // Ensure output dir exists
  if (!existsSync(outputDir)) {
    mkdirSync(outputDir, { recursive: true });
    log(quiet, `  Created output directory: ${outputDir}`);
  }

  // Step 5: Write outputs
  log(quiet, "Step 5/5: Writing output files...");

  await Promise.all([
    Bun.write(join(outputDir, "repo.index.json"), JSON.stringify(repoIndex, null, 2)),
    Bun.write(join(outputDir, "repo.track_suggestions.json"), JSON.stringify(trackSuggestions, null, 2)),
    Bun.write(join(outputDir, "repo.ignore_suggestions.json"), JSON.stringify(ignoreSuggestions, null, 2)),
  ]);

  const elapsed = ((performance.now() - startTime) / 1000).toFixed(2);

  log(quiet, `Done in ${elapsed}s`);
  log(quiet, "");

  // Print summary
  if (!quiet) {
    console.log("=== RepoArchitect Index Summary ===");
    console.log(`  Repository:  ${repoPath}`);
    console.log(`  Total files: ${repoIndex.total_files}`);
    console.log(`  Tracked:     ${repoIndex.tracked}`);
    console.log(`  Untracked:   ${repoIndex.untracked}`);
    console.log(`  Track candidates:  ${trackSuggestions.suggestions.length}`);
    console.log(`  Ignore candidates: ${ignoreSuggestions.suggestions.length}`);
    console.log(`  Output dir:  ${outputDir}`);
    console.log(`  Elapsed:     ${elapsed}s`);
    console.log("");
    console.log("  Files written:");
    console.log(`    ${join(outputDir, "repo.index.json")}`);
    console.log(`    ${join(outputDir, "repo.track_suggestions.json")}`);
    console.log(`    ${join(outputDir, "repo.ignore_suggestions.json")}`);
    console.log("");
  }

  // Handle --top-untracked
  if (args.topUntracked !== null) {
    printTopUntracked(trackSuggestions, args.topUntracked, statMap);
  }

  // Handle --apply-ignore
  if (args.applyIgnore) {
    await applyIgnore(repoPath, ignoreSuggestions);
  }
}

main().catch((err) => {
  console.error("Fatal error:", err.message || err);
  process.exit(1);
});
