# Repository Instructions

This repo is for agentic engineering. It includes a Pi package for reusable Pi extensions, Agent Skills, prompt templates, and themes.

## Rules

Authoritative rule files live under `rules/`:

- `rules/package-resources.md` — Pi package manifest, catalog, prompt, and dependency rules.
- `rules/global-skills.md` — global-first daily skill workflow.
- `rules/extensions.md` — extension naming, schema, mutation, and output rules.
- `rules/identity-verification.md` — source-of-truth checks for identity-bearing values.

Quick rules:

- Keep resources discoverable through `package.json#pi`.
- Daily-use skills are global-first: import/test them under `~/.pi/agent/skills/<name>/` first. When sharing one from this repo, keep the real files in `skills/<name>/` and symlink the global skill back to that repo path so Pi loads it globally and `npm pack` includes it. Do not maintain two copies; copies drift.
- Do not commit secrets, API keys, credentials, session files, or machine-local config. A gitleaks pre-commit hook + CI gate enforces this — see "Secret + PII gate" below.
- Extension names and skill names use kebab-case directories.
- Tool names inside extensions should use snake_case and be globally specific, e.g. `agentic_utilities_ping`.
- Skills must follow Agent Skills structure: `skills/<name>/SKILL.md`, frontmatter `name` matching the directory, and a precise `description`.
- For extension string enum parameters, use `StringEnum` from `@mariozechner/pi-ai`, not `Type.Union([Type.Literal(...)])`.
- Custom tools that mutate files must use `withFileMutationQueue()` around the full read-modify-write window.
- Custom tools with large output must truncate output and report where the full output is stored.
- Keep each resource documented in `docs/catalog.md`.
- Verify identifiers before writing them. Before writing any GitHub user/org slug, repo name, package name, version number, or other identity-bearing value into `plugin.json`, `marketplace.json`, READMEs, or other manifests, confirm it with the source-of-truth tool (`gh api user --jq .login`, `git remote -v`, `npm whoami`, `git config user.email`, registry queries). Pattern-matched plausible values are not facts.

## Verification

Run before shipping changes:

```bash
npm run check
npm run pack:dry
pi -e .
```

## Secret + PII gate

This is a public marketplace repo. Anything committed becomes searchable on GitHub the moment it lands on a public branch — even files that look innocent (markdown port-history docs, README examples, output templates) can leak the contributor's private infrastructure topology.

Two-layer enforcement, both running gitleaks 8.x:

1. **Local pre-commit** (`.husky/pre-commit`) runs `gitleaks protect --staged` against `.gitleaks.toml` (tracked, structural patterns) and `.gitleaks.local.toml` (gitignored, per-contributor personal terms). Blocks the commit on any match.
2. **CI** (`.github/workflows/ci.yml`, `secret-scan` job, blocks the `check` job) runs gitleaks across the full diff on every push and PR. Backstop in case the local hook is bypassed.

**For agents (including future me) writing or porting content:**

- Before staging anything markdown-heavy, scan your own output for: home paths (`/Users/<name>/`), RFC1918 IPs (`10.x`, `172.16-31.x`, `192.168.x`), `localhost:<port>`, hardware serials, hardcoded credentials, and any private skill/agent/repo/codename you remember from the source.
- Replace personal references with `${CLAUDE_PLUGIN_ROOT}`, `${HOME}`, generic example values (`192.168.1.50`), or AskUserQuestion prompts.
- Personal port-history docs (CHANGES.md, MIGRATION.md, etc.) are gitignored on purpose — they describe what was scrubbed and frequently leak the very terms they're documenting. Put port history in the PR description instead.
- If gitleaks fires on a real false positive, add a narrow `[allowlist]` entry to `.gitleaks.toml` with a justifying comment. Never bypass with `--no-verify`. The gate exists because the alternative is finding the leak on a public repo two days later.

To extend protection with your own private terms (skill names, internal codenames, vault paths), copy `.gitleaks.local.toml.example` → `.gitleaks.local.toml` and edit. The `.local.toml` file is gitignored and CI never sees it.

## Harness maintenance

Run the weekly or post-failure garbage-collection ritual in `docs/garbage-collection.md` to convert repeated agent friction into rules, lints, docs, tests, prompts, or skills.

<!-- opensrc:start -->

## Source Code Reference

Source code for dependencies is available in `opensrc/` for deeper understanding of implementation details.

See `opensrc/sources.json` for the list of available packages and their versions.

Use this source code when you need to understand how a package works internally, not just its types/interface.

### Fetching Additional Source Code

To fetch source code for a package or repository you need to understand, run:

```bash
npx opensrc <package>           # npm package (e.g., npx opensrc zod)
npx opensrc pypi:<package>      # Python package (e.g., npx opensrc pypi:requests)
npx opensrc crates:<package>    # Rust crate (e.g., npx opensrc crates:serde)
npx opensrc <owner>/<repo>      # GitHub repo (e.g., npx opensrc vercel/ai)
```

<!-- opensrc:end -->
