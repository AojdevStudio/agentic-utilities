## Review
- Correct: The `web-tools.ts` parse issue is fixed in the inspected files. TypeScript parse diagnostics returned `parse ok` for both `~/.pi/agent/extensions/web-tools.ts` and `extensions/web-tools.ts`.
- Correct: Pi-style runtime loading succeeds. Loading `~/.pi/agent/extensions/web-tools.ts` through `@mariozechner/pi-coding-agent/dist/core/extensions/loader.js` returned `errors: []` and registered `web_search`, `tavily_search`, `tavily_crawl`, and `tavily_map`; registrations are at `~/.pi/agent/extensions/web-tools.ts:250`, `:392`, `:489`, and `:569`.
- Correct: The global file's `@sinclair/typebox` import is resolvable in Pi runtime because the loader aliases `@sinclair/typebox` to bundled `typebox` in both Bun virtual modules and Node aliases: `node_modules/@mariozechner/pi-coding-agent/dist/core/extensions/loader.js:29-36`, `:65-76`, `:271-279`.
- Correct: Package discovery includes the repo extension path. `package.json:14-31` includes `extensions` in `files` and `./extensions/*.ts` in `pi.extensions`; `npm pack --dry-run --json` listed `extensions/web-tools.ts`.
- Note: `<repo root>/plan.md` and `<repo root>/progress.md` were not present, so root-cause context from those files could not be read.
- Note: The global and repo copies differ only at the Typebox import: global uses `@sinclair/typebox` at `~/.pi/agent/extensions/web-tools.ts:11`; repo uses `typebox` at `extensions/web-tools.ts:11`. Both loaded successfully through the Pi loader, but the drift is worth syncing later to avoid confusion.
- Note: Direct TypeScript checking of the global file outside Pi's loader is not reliable: raw `tsc` from the repo could not resolve modules relative to `~/.pi/agent/extensions`. This is not a Pi runtime failure because the Pi loader provides aliases.

## Root cause confirmation
- Current root cause status: no remaining syntax/parse root cause found. The file parses cleanly and loads through the same extension loader path Pi uses.
- If the prior failure still appears, the evidence points away from TypeScript syntax and toward stale loaded content, a non-Pi loader path, or a copied/global version mismatch.

## Minimal next steps
1. Reload/restart Pi so it re-reads `~/.pi/agent/extensions/web-tools.ts`.
2. Optionally sync the import line between the repo and global copies (`typebox` vs `@sinclair/typebox`) for consistency.
3. Run a live tool call only after confirming the needed env vars exist (`BRAVE_SEARCH_API_KEY` for `web_search`, `TAVILY_API_KEY` for Tavily tools); missing secrets are intentionally thrown at `~/.pi/agent/extensions/web-tools.ts:66-73` and `:237-247`.
