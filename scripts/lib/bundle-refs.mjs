import fs from "node:fs";
import path from "node:path";

/**
 * Resolve a local bundle reference (e.g. `scripts/gen.sh`) against `base`
 * and report whether it is safe and exists.
 *
 *  - `traversal`: the reference contains a "." or ".." path segment, or its
 *    resolved path falls outside `base` — rejected before ever touching disk.
 *  - `missing`: the reference stays inside `base` but no file exists there.
 *  - ok: the reference stays inside `base` and resolves to an existing file.
 */
export function resolveBundleReference(base, pathPart) {
  if (pathPart.split("/").some((segment) => segment === "." || segment === "..")) {
    return { ok: false, reason: "traversal" };
  }
  const resolved = path.join(base, pathPart);
  const rel = path.relative(base, resolved);
  if (rel.startsWith("..") || path.isAbsolute(rel)) {
    return { ok: false, reason: "traversal" };
  }
  if (!fs.existsSync(resolved)) {
    return { ok: false, reason: "missing", resolved };
  }
  return { ok: true, resolved };
}
