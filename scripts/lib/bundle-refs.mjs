import path from "node:path";

/**
 * Extract every unambiguous local bundle reference from `text`.
 *
 * A reference is unambiguous only when it starts at a token boundary — not
 * preceded by a word/path character, so a match can never "resync" mid-path
 * and swallow part of a larger expression (e.g. `${CLAUDE_PLUGIN_ROOT}/../tools/x.ts`
 * must never be reinterpreted as the bare reference `tools/x.ts`) — and ends
 * at one, so a coincidental prefix of a longer filename (e.g.
 * `assets/logo.png.bak`) can't be truncated into a match for a real asset
 * (`assets/logo.png`) that happens to exist. Leading `./` and `../`
 * segments are captured as part of the reference itself, not silently
 * dropped or left unmatched, so both a safe same-directory prefix and real
 * traversal are visible downstream instead of disappearing.
 */
export function extractBundleReferences(text, { subdirs, extensions }) {
  const pattern = new RegExp(
    `(?<![\\w./-])(\\$\\{CLAUDE_PLUGIN_ROOT\\}/)?((?:\\.{1,2}/)*(?:skills/[a-z0-9-]+/)?(?:${subdirs.join("|")})/[A-Za-z0-9._/-]+\\.(?:${extensions.join("|")}))(?![\\w.-])`,
    "g",
  );
  const seen = new Set();
  const refs = [];
  for (const [, prefix, pathPart] of text.matchAll(pattern)) {
    const prefixed = Boolean(prefix);
    // biome-ignore lint/suspicious/noTemplateCurlyInString: literal "${CLAUDE_PLUGIN_ROOT}" placeholder text, not a JS template
    const ref = (prefixed ? "${CLAUDE_PLUGIN_ROOT}/" : "") + pathPart;
    if (seen.has(ref)) continue;
    seen.add(ref);
    refs.push({ prefixed, pathPart, ref });
  }
  return refs;
}

/** True if any path segment is a ".." traversal component. A lone "." (current dir) is a safe no-op. */
function hasTraversalSegment(pathPart) {
  return pathPart.split("/").some((segment) => segment === "..");
}

/**
 * Resolve a local bundle reference against `base`, purely lexically: no
 * filesystem access. Rejects traversal (".." segments, or a normalized
 * result outside `base`) before any I/O would happen. Callers are
 * responsible for the filesystem-facing checks (existence, regular-file,
 * symlink containment) once a reference resolves `ok`.
 */
export function resolveBundleReference(base, pathPart) {
  if (hasTraversalSegment(pathPart)) {
    return { ok: false, reason: "traversal" };
  }
  const resolved = path.join(base, pathPart);
  const rel = path.relative(base, resolved);
  if (rel.startsWith("..") || path.isAbsolute(rel)) {
    return { ok: false, reason: "traversal" };
  }
  return { ok: true, resolved };
}
