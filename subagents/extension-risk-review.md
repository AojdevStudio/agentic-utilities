# Extension packaging risk review

Scope: reviewed `~/.pi/agent/extensions/web-tools.ts`, `interactive-artifacts/`, `question.ts`, `adversarial-review.ts`, `anti-hedging.ts`, `todos.ts`, plus repo packaging metadata/copies under `extensions/`.

`plan.md` and `progress.md` were requested but are absent (`ENOENT`).

## Review

- Correct: package discovery and pack inclusion look good.
  - `package.json:27-31` includes both flat extension files and `extensions/*/index.ts`.
  - `npm run list` lists `extensions/adversarial-review.ts`, `anti-hedging.ts`, `interactive-artifacts/index.ts`, `question.ts`, `todos.ts`, and `web-tools.ts`.
  - `npm run pack:dry` includes `extensions/interactive-artifacts/public/app.css` and `extensions/interactive-artifacts/public/app.js`, so the browser assets needed by `interactive-artifacts/index.ts:99-100` are packaged.
  - `docs/catalog.md` now documents all reviewed extensions (`docs/catalog.md:8-16`).

- Correct: no real committed API keys/secrets found in reviewed extension files.
  - `web-tools.ts` reads secrets from `process.env` or `${HOME}/.env` (`~/.pi/agent/extensions/web-tools.ts:16,57-70`) and sends Tavily auth only at runtime (`web-tools.ts:237-245`).
  - Artifact/question callback tokens are generated with `randomUUID()` and checked per request (`interactive-artifacts/index.ts:331,487-490`; `question.ts:1370-1373,1016-1021`).
  - Local servers bind to loopback only (`interactive-artifacts/index.ts:628`; `question.ts:1078`).

- Blocker: secret/PII gate currently fails on `interactive-artifacts/public/app.js`.
  - Evidence: `gitleaks detect --no-git --source .../extensions/interactive-artifacts --config .gitleaks.toml --redact=20 --verbose` reports `RuleID: rfc1918-ipv4` at `extensions/interactive-artifacts/public/app.js:17` because `mermaid@<version>` matches the repo’s RFC1918 `10.x.x.x` pattern.
  - Impact: this is a false positive, not a real secret, but it will block the local/CI secret gate unless the CDN version string is changed or narrowly allowlisted.

- Blocker: `npm run typecheck` fails for the package as currently staged.
  - Evidence: `extensions/question.ts:54` and `:88` use `Type.Optional(schema, options)`, but `typebox`’s current signature expects one argument.
  - Evidence: `extensions/adversarial-review.ts:302` passes `createReadOnlyTools(targetDir)` where the installed type expects `string[]`.
  - Evidence: `extensions/todos.ts:796,1842,1847,1966,2105,2151` have strict type errors.
  - Impact: `npm run check` cannot pass, so the extension batch is not package-ready.

- Blocker: Mermaid diagrams in `interactive-artifacts` are blocked by the extension’s own CSP.
  - Evidence: the page CSP allows `script-src 'self' 'unsafe-inline'` only (`~/.pi/agent/extensions/interactive-artifacts/index.ts:457-462`), but the browser app dynamically loads DOMPurify and Mermaid from `https://cdn.jsdelivr.net` (`interactive-artifacts/public/app.js:16-17,154-159,191-193`).
  - Impact: browser Mermaid rendering will fail under CSP; the app falls back to “Mermaid render failed. Showing source.” (`app.js:220-225`).

- Note: global extension files use `@sinclair/typebox`, while repo package copies use `typebox`.
  - Evidence: global `web-tools.ts:11`, `question.ts:3-4`, `todos.ts:21`, `interactive-artifacts/index.ts:8`, and `adversarial-review.ts:10` import `@sinclair/typebox`; repo copies import `typebox` / `typebox/value` (`extensions/web-tools.ts:11`, `question.ts:3-4`, `todos.ts:21`, `interactive-artifacts/index.ts:8`, `adversarial-review.ts:10`).
  - Pi’s loader aliases `@sinclair/typebox` to `typebox` at runtime (`node_modules/@mariozechner/pi-coding-agent/dist/core/extensions/loader.js:30-36,71-76`), so global runtime loading is probably fine. Direct package typechecking depends on the repo copies using compatible `typebox` syntax.

- Note: `question.ts` has optional global runtime dependencies for Mermaid ASCII rendering.
  - Evidence: it shells out to `npm root -g` and tries `beautiful-mermaid` / global `pi-mermaid` locations (`~/.pi/agent/extensions/question.ts:247-267`). Missing renderer degrades gracefully with a visual-unavailable message (`question.ts:300`).
  - Impact: not a packaging blocker unless ASCII Mermaid rendering is expected to work from the package without global installs.

- Note: `web-tools.ts` may expose the user’s home path in runtime error text.
  - Evidence: `ENV_FILE = join(homedir(), ".env")` (`web-tools.ts:16`) is interpolated into missing-credential errors (`web-tools.ts:66-70`).
  - Impact: no committed absolute path, but failed tool calls can put `/Users/<user>/.env` into chat/session logs.

Verification run:

```text
npm run list        # pass; all reviewed extensions discovered
npm run pack:dry    # pass; interactive-artifacts public assets included
npm run typecheck   # fail; errors listed above
gitleaks detect --no-git --source extensions/interactive-artifacts --config .gitleaks.toml --redact=20 --verbose  # fail; false-positive mermaid@<version>
```
