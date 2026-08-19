# CI Templates

Handlebars-over-YAML templates consumed by the `CISetup` workflow (`../../workflows/CISetup.md`).

## Variable conventions

| Style | Meaning | Substituted at |
|-------|---------|----------------|
| `{{var}}` | Handlebars — substituted at scaffold time by CISetup | Skill execution |
| `${{ expr }}` | GitHub Actions expression — preserved verbatim in output YAML | GitHub Actions runtime |
| `{{#eq x "y"}}…{{/eq}}` | Handlebars conditional block | Skill execution |
| `{{#if x}}…{{/if}}` | Handlebars conditional block | Skill execution |

## Shipped templates (v1)

| File | Purpose | Default runner |
|------|---------|----------------|
| `node-pr-gate.yml.hbs` | Typecheck + lint + test + build smoke (Node/bun/pnpm/yarn/npm) | `vars.SELF_HOSTED_LINUX` → `[self-hosted, homelab-ci]` |
| `gitleaks.yml.hbs` | Secret scan on every PR + push | `vars.SELF_HOSTED_LINUX` |
| `tauri-macos-build.yml.hbs` | Tauri macOS build (with optional Apple signing) | `vars.SELF_HOSTED_MACOS` → `[self-hosted, homelab-macos, mac-mini-m4]` |
| `runner-health-check.yml.hbs` | Cron probe that updates `SELF_HOSTED_*_AVAILABLE` org Variables | `ubuntu-latest` (must be hosted) |

## Deferred templates (to be authored on first invocation that needs them)

- `drizzle-migrate-diff.yml.hbs` — `drizzle-kit generate` dry-run + PR comment
- `prisma-migrate-diff.yml.hbs` — `prisma migrate diff` against shadow DB
- `playwright-e2e.yml.hbs` — Playwright against preview URL
- `wrangler-deploy-dry-run.yml.hbs` — Cloudflare Workers dry-run
- `release-notes.yml.hbs` — Tag-triggered changelog generation
- `node-deploy-vercel.yml.hbs` — Vercel deploy + preview comment

Adding a template: drop the `.yml.hbs` file here, then update `../../workflows/CISetup.md` § "Templates Inventory" table with the row, and add a Phase 3 parameter question.

## Conventions

- **Always** wrap `runs-on` with `${{ fromJSON(vars.SELF_HOSTED_X || '[…]') }}` — never hardcode self-hosted labels.
- **Always** carve out fork PRs with `if: github.event.pull_request.head.repo.full_name == github.repository` for any job that targets self-hosted runners; route fork PRs to `ubuntu-latest`/`macos-latest`.
- **Always** include `timeout-minutes` (self-hosted runners can hang on stuck jobs forever otherwise).
- **Always** include `concurrency:` with `cancel-in-progress: true` for PR-triggered workflows — prevents stacked runs on rapid pushes.
- **Avoid** `secrets: inherit`. Pass secrets explicitly via `env:` blocks scoped to the step that needs them.
- **Avoid** mutating shared state on self-hosted runners (caches, global npm installs). Workspaces should be ephemeral within the job.
