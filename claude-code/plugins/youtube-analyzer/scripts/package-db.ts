#!/usr/bin/env bun

// PackageDb.ts — CRUD for tutorial package/version tracking
// Usage:
//   bun run PackageDb.ts add --name next --display-name "Next.js" --version-mentioned "14.2" --category framework --source "https://youtube.com/watch?v=abc"
//   bun run PackageDb.ts list
//   bun run PackageDb.ts query --name next
//   bun run PackageDb.ts refresh --stale-days 7
//
// Database: ~/.config/youtube-analyzer/package-db.json

import { existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";

const DB_PATH = join(homedir(), ".config", "youtube-analyzer", "package-db.json");
const DB_DIR = join(homedir(), ".config", "youtube-analyzer");

// Types
interface PackageEntry {
  name: string;
  displayName: string;
  versionMentioned: string;
  latestVersion: string;
  latestChecked: string; // ISO date
  category: "framework" | "library" | "tool" | "service" | "language" | "database" | "other";
  sourceVideos: string[];
  notes: string;
}

interface PackageDb {
  lastUpdated: string; // ISO datetime
  packages: PackageEntry[];
}

// Helpers
async function ensureDbDir(): Promise<void> {
  if (!existsSync(DB_DIR)) {
    await mkdir(DB_DIR, { recursive: true });
  }
}

async function loadDb(): Promise<PackageDb> {
  await ensureDbDir();

  if (!existsSync(DB_PATH)) {
    return {
      lastUpdated: new Date().toISOString(),
      packages: [],
    };
  }

  const file = Bun.file(DB_PATH);
  const text = await file.text();
  return JSON.parse(text);
}

async function saveDb(db: PackageDb): Promise<void> {
  await ensureDbDir();
  db.lastUpdated = new Date().toISOString();
  await Bun.write(DB_PATH, JSON.stringify(db, null, 2));
}

// Commands
async function addPackage(args: any): Promise<void> {
  const {
    name,
    "display-name": displayName,
    "version-mentioned": versionMentioned,
    category,
    source,
    "latest-version": latestVersion,
    notes,
  } = args;

  const db = await loadDb();
  const existingIndex = db.packages.findIndex((p) => p.name.toLowerCase() === name.toLowerCase());

  if (existingIndex >= 0) {
    // Update existing package (allows partial updates)
    const existing = db.packages[existingIndex];

    // Add source if provided and not already present
    if (source && !existing.sourceVideos.includes(source)) {
      existing.sourceVideos.push(source);
    }

    // Update version if provided and different
    if (versionMentioned && versionMentioned !== existing.versionMentioned) {
      existing.versionMentioned = versionMentioned;
    }

    // Update latest version if provided
    if (latestVersion) {
      existing.latestVersion = latestVersion;
      existing.latestChecked = new Date().toISOString().split("T")[0];
    }

    // Update notes if provided
    if (notes !== undefined) {
      existing.notes = notes;
    }

    await saveDb(db);
    console.log(JSON.stringify(existing, null, 2));
  } else {
    // Create new package (requires all fields)
    if (!name || !displayName || !versionMentioned || !category || !source) {
      console.error(
        JSON.stringify(
          {
            error: "Missing required arguments for new package",
            required: ["name", "display-name", "version-mentioned", "category", "source"],
            usage:
              "bun run PackageDb.ts add --name <name> --display-name <display> --version-mentioned <version> --category <category> --source <url>",
          },
          null,
          2,
        ),
      );
      process.exit(1);
    }

    const validCategories = ["framework", "library", "tool", "service", "language", "database", "other"];
    if (!validCategories.includes(category)) {
      console.error(
        JSON.stringify(
          {
            error: `Invalid category: ${category}`,
            validCategories,
          },
          null,
          2,
        ),
      );
      process.exit(1);
    }

    const newPackage: PackageEntry = {
      name,
      displayName,
      versionMentioned,
      latestVersion: latestVersion || "",
      latestChecked: latestVersion ? new Date().toISOString().split("T")[0] : "",
      category,
      sourceVideos: [source],
      notes: notes || "",
    };

    db.packages.push(newPackage);
    await saveDb(db);
    console.log(JSON.stringify(newPackage, null, 2));
  }
}

async function listPackages(): Promise<void> {
  const db = await loadDb();

  if (db.packages.length === 0) {
    console.log("Package Database (0 packages, last updated: never)");
    console.log("No packages tracked yet.");
    console.log(JSON.stringify({ packages: [], total: 0 }, null, 2));
    return;
  }

  // Print table
  const lastUpdatedDate = new Date(db.lastUpdated).toISOString().split("T")[0];
  console.log(`Package Database (${db.packages.length} packages, last updated: ${lastUpdatedDate})`);
  console.log("┌──────────────────┬──────────────────┬──────────────┬──────────────┬────────────┬─────────┐");
  console.log("│ Name             │ Display Name     │ Version (Vid)│ Latest Ver   │ Category   │ Sources │");
  console.log("├──────────────────┼──────────────────┼──────────────┼──────────────┼────────────┼─────────┤");

  for (const pkg of db.packages) {
    const name = pkg.name.padEnd(16).substring(0, 16);
    const displayName = pkg.displayName.padEnd(16).substring(0, 16);
    const versionMentioned = pkg.versionMentioned.padEnd(13).substring(0, 13);
    const latestVersion = pkg.latestVersion.padEnd(13).substring(0, 13);
    const category = pkg.category.padEnd(10).substring(0, 10);
    const sources = pkg.sourceVideos.length.toString().padStart(7);

    console.log(`│ ${name} │ ${displayName} │ ${versionMentioned} │ ${latestVersion} │ ${category} │${sources} │`);
  }

  console.log("└──────────────────┴──────────────────┴──────────────┴──────────────┴────────────┴─────────┘");
  console.log();

  // Also output JSON
  console.log(
    JSON.stringify(
      {
        packages: db.packages,
        total: db.packages.length,
        lastUpdated: db.lastUpdated,
      },
      null,
      2,
    ),
  );
}

async function queryPackage(args: any): Promise<void> {
  const { name } = args;

  if (!name) {
    console.error(
      JSON.stringify(
        {
          error: "Missing required argument: name",
          usage: "bun run PackageDb.ts query --name <name>",
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const db = await loadDb();
  const pkg = db.packages.find((p) => p.name.toLowerCase() === name.toLowerCase());

  if (!pkg) {
    // Find closest match
    const closest = db.packages
      .map((p) => ({
        name: p.name,
        distance: levenshteinDistance(name.toLowerCase(), p.name.toLowerCase()),
      }))
      .sort((a, b) => a.distance - b.distance)[0];

    console.error(
      JSON.stringify(
        {
          error: `Package '${name}' not found`,
          suggestion: closest ? closest.name : null,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  console.log(JSON.stringify(pkg, null, 2));
}

async function refreshStale(args: any): Promise<void> {
  const staleDays = args["stale-days"] || 7;
  const db = await loadDb();

  const now = new Date();
  const stalePackages = db.packages
    .map((pkg) => {
      if (!pkg.latestChecked) {
        return {
          name: pkg.name,
          displayName: pkg.displayName,
          lastChecked: "",
          daysStale: -1,
        };
      }

      const lastChecked = new Date(pkg.latestChecked);
      const daysDiff = Math.floor((now.getTime() - lastChecked.getTime()) / (1000 * 60 * 60 * 24));

      if (daysDiff >= staleDays) {
        return {
          name: pkg.name,
          displayName: pkg.displayName,
          lastChecked: pkg.latestChecked,
          daysStale: daysDiff,
        };
      }

      return null;
    })
    .filter((p) => p !== null);

  console.log(
    JSON.stringify(
      {
        stalePackages,
        totalStale: stalePackages.length,
        totalPackages: db.packages.length,
        staleDaysThreshold: staleDays,
      },
      null,
      2,
    ),
  );
}

// Levenshtein distance for fuzzy matching
function levenshteinDistance(a: string, b: string): number {
  const matrix: number[][] = [];

  for (let i = 0; i <= b.length; i++) {
    matrix[i] = [i];
  }

  for (let j = 0; j <= a.length; j++) {
    matrix[0][j] = j;
  }

  for (let i = 1; i <= b.length; i++) {
    for (let j = 1; j <= a.length; j++) {
      if (b.charAt(i - 1) === a.charAt(j - 1)) {
        matrix[i][j] = matrix[i - 1][j - 1];
      } else {
        matrix[i][j] = Math.min(matrix[i - 1][j - 1] + 1, matrix[i][j - 1] + 1, matrix[i - 1][j] + 1);
      }
    }
  }

  return matrix[b.length][a.length];
}

// Main
async function main() {
  const { values, positionals } = parseArgs({
    args: process.argv.slice(2),
    options: {
      name: { type: "string" },
      "display-name": { type: "string" },
      "version-mentioned": { type: "string" },
      "latest-version": { type: "string" },
      category: { type: "string" },
      source: { type: "string" },
      notes: { type: "string" },
      "stale-days": { type: "string" },
    },
    allowPositionals: true,
  });

  const command = positionals[0];

  // Convert stale-days to number
  const args = {
    ...values,
    "stale-days": values["stale-days"] ? parseInt(values["stale-days"], 10) : undefined,
  };

  switch (command) {
    case "add":
      await addPackage(args);
      break;
    case "list":
      await listPackages();
      break;
    case "query":
      await queryPackage(args);
      break;
    case "refresh":
      await refreshStale(args);
      break;
    default:
      console.error(
        JSON.stringify(
          {
            error: "Unknown command",
            usage: "bun run PackageDb.ts <add|list|query|refresh> [options]",
            commands: {
              add: "Add or update a package",
              list: "List all packages",
              query: "Query a specific package",
              refresh: "Find packages with stale version checks",
            },
          },
          null,
          2,
        ),
      );
      process.exit(1);
  }
}

main().catch((err) => {
  console.error(
    JSON.stringify(
      {
        error: err.message,
        stack: err.stack,
      },
      null,
      2,
    ),
  );
  process.exit(1);
});
