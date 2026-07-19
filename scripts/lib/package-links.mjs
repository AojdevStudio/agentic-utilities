import path from "node:path";

function headingAnchors(content) {
  return new Set(
    content
      .split("\n")
      .map((line) => line.match(/^#{1,6}\s+(.+)$/)?.[1])
      .filter(Boolean)
      .map((heading) =>
        heading
          .toLowerCase()
          .replace(/[`*_~]/g, "")
          .replace(/[^\p{L}\p{N}\s-]/gu, "")
          .trim()
          .replace(/\s+/g, "-"),
      ),
  );
}

export function findBrokenPackedLinks({ sourcePath, content, packedPathSet, readTarget }) {
  const broken = [];
  for (const match of content.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
    const target = match[1].trim().split(/\s+/, 1)[0];
    if (/^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
    const [relativeTarget, anchor] = target.split("#", 2);
    const targetPath = relativeTarget
      ? path.posix.normalize(path.posix.join(path.posix.dirname(sourcePath), relativeTarget))
      : sourcePath;
    if (!packedPathSet.has(targetPath)) {
      broken.push(`${sourcePath} -> ${target}`);
      continue;
    }
    if (anchor && !headingAnchors(readTarget(targetPath)).has(anchor)) {
      broken.push(`${sourcePath} -> #${anchor}`);
    }
  }
  return broken;
}
