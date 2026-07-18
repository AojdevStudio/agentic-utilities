import { parse as parseYaml } from "yaml";

/**
 * Parse a leading `--- ... ---` YAML frontmatter block into a mapping.
 * Throws when the block is absent, unterminated, not valid YAML, or does
 * not parse to a mapping (e.g. a bare scalar or sequence document).
 */
// The closing delimiter must be an exact "---" line (optionally followed by
// trailing spaces/tabs before the newline or end of string) — not merely a
// line that starts with "---". Without this, "---not-a-delimiter", "----",
// and "--- trailing junk" all satisfy a naive `indexOf("\n---")` search and
// silently close the block early or swallow content past the real closer.
const CLOSING_DELIMITER = /^---\n([\s\S]*?)\n---[ \t]*(?:\n|$)/;

export function parseFrontmatter(content, filePath) {
  const normalized = content.replace(/\r\n/g, "\n");
  if (!normalized.startsWith("---\n")) {
    throw new Error(`${filePath}: missing YAML frontmatter block`);
  }
  const match = CLOSING_DELIMITER.exec(normalized);
  if (!match) {
    throw new Error(`${filePath}: unterminated YAML frontmatter block`);
  }
  const raw = match[1];
  let data;
  try {
    data = parseYaml(raw);
  } catch (err) {
    throw new Error(`${filePath}: frontmatter is not valid YAML: ${err.message}`);
  }
  if (data === null || typeof data !== "object" || Array.isArray(data)) {
    throw new Error(`${filePath}: frontmatter must be a YAML mapping`);
  }
  return data;
}

/** True only for a trimmed, non-empty string value (rejects null, numbers, collections). */
export function isNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}
