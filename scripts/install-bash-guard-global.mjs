#!/usr/bin/env node
import { lstat, mkdir, rm, symlink } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

const repoRoot = resolve(new URL("..", import.meta.url).pathname);
const source = join(repoRoot, "extensions", "bash-guard");
const target = join(homedir(), ".pi", "agent", "extensions", "bash-guard");

await mkdir(dirname(target), { recursive: true });

try {
  const stat = await lstat(target);
  if (stat.isSymbolicLink()) {
    await rm(target);
  } else {
    console.error(`Refusing to replace non-symlink: ${target}`);
    process.exit(1);
  }
} catch (error) {
  if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") throw error;
}

await symlink(source, target, "dir");
console.log(`Installed bash-guard globally: ${target} -> ${source}`);
console.log("Run /reload in Pi to load it in an existing session.");
