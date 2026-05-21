# TypeScript / Node Stack

Covers: Node, Deno, Bun. Most patterns assume Node + npm/pnpm/yarn or Bun.

## Tooling matrix

| Concern | Recommended | Alternatives |
|---------|-------------|--------------|
| Lint + format | Biome (single tool, fast) | ESLint + Prettier (more rules, more config) |
| Test runner | Vitest (TS-native, Vite-compat) | Jest (mature), `bun test` (Bun-native), `node:test` |
| Type checker | `tsc --noEmit` | tsgo (10× faster, experimental) |
| API docs | TypeDoc + custom missing-docs report | `eslint-plugin-jsdoc`, API Extractor, package-specific docs |
| Pre-commit | Husky + lint-staged | lefthook (Go-based, parallel) |
| E2E | Playwright | Cypress |
| Package manager | Detect from lockfile | bun.lockb / pnpm-lock.yaml / yarn.lock / package-lock.json |

## Lint config paths to check

- `biome.json`, `biome.jsonc` (Biome 1.x), `biome.json` with `$schema: "https://biomejs.dev/schemas/2.x.x/schema.json"` (Biome 2.x)
- `eslint.config.js`, `eslint.config.mjs`, `eslint.config.ts` (flat config)
- `.eslintrc`, `.eslintrc.js`, `.eslintrc.json` (legacy)
- `.prettierrc*`
- `oxlint.json` (oxlint, Rust-based, very fast)

## Test runner detection

- `vitest.config.*`, `vite.config.*` with vitest plugin → Vitest
- `jest.config.*` or `jest` key in package.json → Jest
- `bun test` referenced in package.json scripts → Bun native
- `node --test` → Node native runner

## API documentation policy checks

For TypeScript repos with exported package APIs, check for:

- `typedoc.json` or `typedoc` config in `package.json`
- `package.json` scripts such as `docs:api`, `docs:api:check`, `docs:api:report`, or `docs:api:strict`
- `docs/api-documentation-policy.md` or `rules/api-documentation.md`
- `docs/adr/*api*documentation*` or `docs/decisions/*api*documentation*`
- a generated missing-docs report, usually `docs/api-documentation-report.md`
- optional lint support through `eslint-plugin-jsdoc` or a custom rule when the repo already uses ESLint

Recommended shape for Bun/TypeScript packages:

```json
{
  "scripts": {
    "docs:api": "typedoc --options typedoc.json",
    "docs:api:check": "typedoc --options typedoc.json --emit none",
    "docs:api:report": "bun scripts/api-docs-report.ts --write docs/api-documentation-report.md",
    "docs:api:strict": "bun scripts/api-docs-report.ts --fail-on-missing && typedoc --options typedoc.json --emit none --treatValidationWarningsAsErrors"
  }
}
```

Do not wire `docs:api:strict` into CI while the baseline report still has known debt. The useful first step is making the debt explicit and giving agents a contract for documenting new or changed exports.

## Pre-commit pattern (bun + Biome)

```sh
# Install
bun add -D husky lint-staged
bunx husky init
```

`package.json`:
```json
{
  "scripts": {
    "prepare": "husky",
    "typecheck": "tsc --noEmit"
  },
  "lint-staged": {
    "*.{ts,tsx,js,jsx,json}": ["bunx @biomejs/biome check --write --no-errors-on-unmatched"]
  }
}
```

`.husky/pre-commit`:
```sh
#!/usr/bin/env sh
bunx lint-staged
bun run typecheck
```

If typecheck >10s on a fresh checkout, demote to pre-push instead. Pre-commit needs to stay snappy.

## Test wrapper

For most TS projects, `npm test` / `bun test` is already the one-liner. The gap is usually that the script in `package.json` only runs a subset:

```json
"scripts": {
  "test": "vitest run packages/data packages/briefing"  // ← covers 2 of 7 packages
}
```

When auditing, check whether `npm test` actually exercises ALL packages. If not, that's a gap.

For monorepos: `turbo test`, `pnpm -r test`, `bun --filter '*' test` are the right shape.

## CI pattern (GitHub Actions)

Minimal CI that's enough to gate PRs:

```yaml
name: CI
on: [push, pull_request]
jobs:
  lint-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2  # or actions/setup-node@v4
      - run: bun install --frozen-lockfile
      - run: bun run lint
      - run: bun run typecheck
      - run: bun test
```

## Common gotchas to flag

- **Biome `--write .` from monorepo root** — destructive in some configs (cf. Finance-Guru-v2 gotcha #8). Always scope to staged files in pre-commit.
- **`any` proliferation** — TS strict not actually strict if `any` is everywhere. Check `tsconfig.json#strict` AND grep for `any` density.
- **No `noUncheckedIndexedAccess`** — for data-heavy apps (financial, ML), this is a meaningful safety gap even with `strict: true`.
- **Lockfile committed but ignored in CI** — `bun install` without `--frozen-lockfile` lets versions drift between dev and CI.
- **Workspaces with no shared tsconfig** — each package has divergent compiler options, agent edits one config and breaks another.
- **Doc coverage warning treated as mystery failure** — if CodeRabbit or CI flags missing JSDoc/TSDoc, check whether the repo has a policy, ADR, and generated backlog before asking agents to blindly edit every export.

## Custom rule patterns worth encoding

When auditing custom Biome / ESLint rules, recommend these patterns specifically:

- `noRestrictedImports` — block direct imports that bypass an architectural boundary (e.g., `node:fs` from browser code, direct DB client from UI layer)
- `noRestrictedSyntax` — block direct constructor calls that should go through a factory
- `noConsole` (override `console.warn` allowed) — force agents to use the project logger

## Bonus signals (for the audit prompt)

- Monorepo or single-package? (`pnpm-workspace.yaml`, `turbo.json`, `nx.json`, `bun workspaces`)
- TS strictness (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`)
- Package count
- Path aliases / module resolution custom config
- Tauri / Electron / Next.js / specific framework hints
- API documentation contract (`typedoc.json`, `docs:api*` scripts, `docs/api-documentation-policy.md`, ADR/report)
