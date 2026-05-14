# Packaging context: confirmed Pi resources

Strongest finding: target resources are already present in the worktree as untracked files. Repo copies match `~/.pi` for all target skills/extensions except two intentional-looking import normalizations from `@sinclair/typebox` to `typebox` in `extensions/adversarial-review.ts` and `extensions/interactive-artifacts/index.ts`. Packaging is not ship-ready: `npm run lint` and `npm run typecheck` currently fail.

## Requested scope

Add skills from `~/.pi/agent/skills`:

- `adversarial-review`
- `art`
- `awesome-readme`
- `deep-dive`
- `find-docs`
- `gitworkflow`
- `grill-me`
- `harness-worktrees`

Add extensions from `~/.pi/agent/extensions`:

- `web-tools.ts`
- `interactive-artifacts/`
- `question.ts`
- `adversarial-review.ts`
- `anti-hedging.ts`
- `todos.ts`

## Relevant repo rules and discovery behavior

- `package.json:27-40` already loads both flat extension files and directory extensions:
  - `./extensions/*.ts`
  - `./extensions/*/index.ts`
  - `./skills/**/SKILL.md`
- `package.json:14-25` includes `extensions` and `skills` in npm package files.
- `scripts/list-resources.mjs:41-49` discovers extension resources as either `extensions/*.ts` or `extensions/*/index.ts`.
- `scripts/list-resources.mjs:52-56` discovers skills by walking `skills/**/SKILL.md` and follows symlinks safely via realpath tracking at `scripts/list-resources.mjs:14-39`.
- `rules/package-resources.md:5-9`: `package.json#pi` is source of truth, every shared resource must be in `docs/catalog.md`, runtime deps in `dependencies`, Pi-provided packages in `peerDependencies`, validate with `npm run check`, `npm run pack:dry`, `pi -e .`.
- `rules/global-skills.md:5-9`: daily-use skills are global-first, canonical files should live in `skills/<name>/`, and `~/.pi/agent/skills/<name>` should symlink back to repo path after sharing. Current global target skill paths are real dirs, not symlinks.
- `rules/extensions.md:5-11`: kebab-case dirs, default export function, globally specific snake_case tool names, `StringEnum` for string enums, file-mutating tools must wrap full read-modify-write in `withFileMutationQueue()`, truncate large output, check `ctx.hasUI` for interactive UI behavior.
- `rules/identity-verification.md:5-8`: verify identity-bearing values before manifests/READMEs/package metadata/install commands.

## Current resource layout

Existing repo resources before/within current worktree:

- Extensions from `npm run -s list`: `extensions/adversarial-review.ts`, `extensions/anti-hedging.ts`, `extensions/autopilot/index.ts`, `extensions/bash-guard/index.ts`, `extensions/hello/index.ts`, `extensions/interactive-artifacts/index.ts`, `extensions/question.ts`, `extensions/todos.ts`, `extensions/web-tools.ts`.
- Skills from `npm run -s list`: target skills plus `bambu-slicer`, `harness-audit`, `scaffold-notes`.
- Target files are currently untracked per `git status --short`:
  - `?? extensions/adversarial-review.ts`
  - `?? extensions/anti-hedging.ts`
  - `?? extensions/interactive-artifacts/`
  - `?? extensions/question.ts`
  - `?? extensions/todos.ts`
  - `?? extensions/web-tools.ts`
  - `?? skills/adversarial-review/`
  - `?? skills/art/`
  - `?? skills/awesome-readme/`
  - `?? skills/deep-dive/`
  - `?? skills/find-docs/`
  - `?? skills/gitworkflow/`
  - `?? skills/grill-me/`
  - `?? skills/harness-worktrees/`

## Exact copy plan

Use repo root: `<repo root>`.

Skills: copy each whole directory from global to repo, deleting stale destination files and excluding generated deps.

```bash
cd <repo root>
for name in adversarial-review art awesome-readme deep-dive find-docs gitworkflow grill-me harness-worktrees; do
  rsync -a --delete --exclude 'node_modules/' "$HOME/.pi/agent/skills/$name/" "skills/$name/"
done
```

Extensions: copy flat files and directory extension.

```bash
cd <repo root>
cp "$HOME/.pi/agent/extensions/web-tools.ts" extensions/web-tools.ts
cp "$HOME/.pi/agent/extensions/question.ts" extensions/question.ts
cp "$HOME/.pi/agent/extensions/adversarial-review.ts" extensions/adversarial-review.ts
cp "$HOME/.pi/agent/extensions/anti-hedging.ts" extensions/anti-hedging.ts
cp "$HOME/.pi/agent/extensions/todos.ts" extensions/todos.ts
rsync -a --delete "$HOME/.pi/agent/extensions/interactive-artifacts/" extensions/interactive-artifacts/
```

Then normalize Typebox imports if copying from global overwrites the current repo fixes:

```bash
perl -0pi -e 's/from "@sinclair\/typebox"/from "typebox"/g' \
  extensions/adversarial-review.ts \
  extensions/interactive-artifacts/index.ts
```

Global-skill symlink follow-up required by repo rules, but this mutates `~/.pi`; get explicit approval before doing it. Safe backup form:

```bash
for name in adversarial-review art awesome-readme deep-dive find-docs gitworkflow grill-me harness-worktrees; do
  mv "$HOME/.pi/agent/skills/$name" "$HOME/.pi/agent/skills/$name.backup-$(date +%Y%m%d%H%M%S)"
  ln -s "<repo root>/skills/$name" "$HOME/.pi/agent/skills/$name"
done
```

## Copy verification already performed

`diff -qr` repo vs global:

- All target skills identical except `skills/art/Tools/node_modules` exists only in global and should stay excluded.
- Flat extensions identical: `web-tools.ts`, `question.ts`, `anti-hedging.ts`, `todos.ts`.
- Differences:
  - `extensions/adversarial-review.ts:10` repo imports `Type` from `typebox`; global imports `@sinclair/typebox`.
  - `extensions/interactive-artifacts/index.ts:8` repo imports `Type` from `typebox`; global imports `@sinclair/typebox`.

## Target skill details for catalog

Frontmatter evidence:

- `skills/adversarial-review/SKILL.md:2-3`: name `adversarial-review`; deep implementation review that hunts for real bugs.
- `skills/art/SKILL.md:2-3`: name `art`; creates visual assets, diagrams, infographics, thumbnails, icons, comics, screenshots, taxonomies, timelines, maps, comparisons, dashboards.
- `skills/awesome-readme/SKILL.md:2-3`: name `awesome-readme`; creates/improves/reviews GitHub READMEs with story-first structure.
- `skills/deep-dive/SKILL.md:2-11`: name `deep-dive`; structured technical/operational/strategic deep-dive analysis.
- `skills/find-docs/SKILL.md:2-3`: name `find-docs`; retrieves authoritative current technical docs/API refs/examples.
- `skills/gitworkflow/SKILL.md:2-5`: name `gitworkflow`; Git Flow branching, CI monitoring, auto-merge, submodules, issue routing, deploy-workflow isolation.
- `skills/grill-me/SKILL.md:2-3`: name `grill-me`; relentlessly interviews/stress-tests plan or design.
- `skills/harness-worktrees/SKILL.md:2-3`: name `harness-worktrees`; reset/refresh/sync current harness worktree to latest main.

Files to include:

- `skills/adversarial-review/SKILL.md`, `skills/adversarial-review/references/prompt-template.md`
- `skills/art/**` including `.env.example`, `Examples/*.png`, `Lib/*.ts`, `Tools/*`, `Workflows/*.md`, `references/ArtistContext.md`; exclude `Tools/node_modules/`.
- `skills/awesome-readme/SKILL.md`, `skills/awesome-readme/workflows/{Analyze,Create,Improve}.md`
- `skills/deep-dive/SKILL.md`
- `skills/find-docs/SKILL.md`
- `skills/gitworkflow/SKILL.md`, `AGENT.md`, `templates/*`, `workflows/*`
- `skills/grill-me/SKILL.md`
- `skills/harness-worktrees/SKILL.md`, `scripts/reset-worktree-to-main.sh`

## Target extension details for catalog

- `extensions/web-tools.ts:250-571`: registers tools `web_search`, `tavily_search`, `tavily_crawl`, `tavily_map`. Uses Brave/Google CSE/Tavily APIs and truncates large output with Pi helpers at `web-tools.ts:81-97`.
- `extensions/interactive-artifacts/index.ts:765-872`: registers commands `artifact-explain`, `artifact-open` and tools `artifact_publish`, `artifact_get`; serves browser artifact UI with `public/app.js` and `public/app.css`.
- `extensions/question.ts:1129-1344`: registers tools `AskUserQuestion` and `AskBatchQuestions`; supports single-choice, multi-select, text, recommendations, and Mermaid visual context.
- `extensions/adversarial-review.ts:236-264`: registers `adversarial_review` tool; uses read-only Pi agent session and `StringEnum` for `thinkingLevel`.
- `extensions/anti-hedging.ts:23-68`: injects anti-hedging system rules, warns on filler patterns, command `/antihedging`.
- `extensions/todos.ts:1506-1876`: registers `todo` tool and `/todos` TUI command; stores markdown/JSON-frontmatter todos under `.pi/todos` or `PI_TODO_PATH`.

## Catalog update requirements

`docs/catalog.md` currently has rows for `autopilot`, `bash-guard`, `bambu-slicer`, `hello`, prompt, `harness-audit`, `scaffold-notes`, Claude Code plugins. It does **not** have rows for the requested Pi resources except the unrelated `awesome-readme` Claude Code Plugin row.

Add these rows to the main table, preferably grouped by type/name:

```md
| `adversarial-review` | Extension | `extensions/adversarial-review.ts` | active | Runs a separate read-only Pi reviewer model to perform adversarial implementation audits against a plan/spec or production-readiness criteria. |
| `anti-hedging` | Extension | `extensions/anti-hedging.ts` | active | Adds proactive anti-hedging behavior rules and warns when assistant responses end with optional-offer filler. |
| `interactive-artifacts` | Extension | `extensions/interactive-artifacts/index.ts` | active | Publishes browser-based concept explainer artifacts, supports comments, and lets agents update/open active artifacts. |
| `question` | Extension | `extensions/question.ts` | active | Provides interactive single-choice, multi-select, text, batch, and Mermaid-aided user question tools. |
| `todos` | Extension | `extensions/todos.ts` | active | File-backed markdown todos with JSON front matter, optional locks, assignment, garbage collection, and a TUI manager. |
| `web-tools` | Extension | `extensions/web-tools.ts` | active | Web search and crawling tools backed by Brave Search, Google CSE fallback, and Tavily search/crawl/map APIs. |
| `adversarial-review` | Skill | `skills/adversarial-review/SKILL.md` | active | Uses the adversarial review extension for tough read-only implementation audits that hunt real bugs before shipping. |
| `art` | Skill | `skills/art/SKILL.md` | active | Pi-native visual asset workflow library for diagrams, illustrations, thumbnails, infographics, icons, comics, screenshots, maps, timelines, and image prompts. |
| `awesome-readme` | Skill | `skills/awesome-readme/SKILL.md` | active | Story-first README creation, improvement, and analysis workflows for clearer, more compelling GitHub projects. |
| `deep-dive` | Skill | `skills/deep-dive/SKILL.md` | active | Structured, opinionated deep-dive analysis for technical, operational, and strategic topics with context and live-doc grounding. |
| `find-docs` | Skill | `skills/find-docs/SKILL.md` | active | Retrieves authoritative current docs, API references, configuration details, and examples using Context7. |
| `gitworkflow` | Skill | `skills/gitworkflow/SKILL.md` | active | Smart Git workflow guidance for branches, commits, PRs, CI monitoring, merges, releases, submodules, issue routing, and deploy isolation. |
| `grill-me` | Skill | `skills/grill-me/SKILL.md` | active | Relentlessly questions and stress-tests plans/designs until assumptions and decisions are resolved. |
| `harness-worktrees` | Skill | `skills/harness-worktrees/SKILL.md` | active | Maintains parallel harness worktrees by resetting, refreshing, and syncing current worktrees to latest main. |
```

No `package.json#pi` changes are needed for these resources because current glob patterns already discover all requested paths.

## Validation commands

Run from repo root.

```bash
npm run list
npm run lint
npm run typecheck
npm run check
npm run pack:dry
npm pack --dry-run --json
pi -e .
gitleaks protect --staged --config .gitleaks.toml --config .gitleaks.local.toml
```

Known local validation results from this inspection:

- `npm run -s list` passed and listed all requested target resources.
- `npm pack --dry-run --json` passed: `agentic-utilities@0.1.0`, 131 files, 8,584,366 unpacked bytes. It includes `extensions/interactive-artifacts/public/app.{css,js}`, `skills/art/.env.example`, and `skills/art/Examples/*.png`; no copied `node_modules` surfaced.
- `npm run -s lint` failed. Target-scope examples:
  - `skills/art/Tools/GenerateMidjourneyImage.ts`: missing `parseInt(..., 10)` radix at lines 90, 91, 235, 244, 248, 261.
  - `extensions/interactive-artifacts/public/app.js:3`: optional-chain lint.
  - `skills/art/Lib/discord-bot.ts` and `skills/art/Lib/midjourney-client.ts`: import-type/optional-chain/no-implicit-any issues.
  - There are also existing out-of-scope lint failures in `claude-code/plugins/bambu-slicer/cli/*` for missing `node:` protocol imports.
- `npm run -s typecheck` failed. Exact errors:
  - `extensions/adversarial-review.ts(302,9): Type 'Tool[]' is not assignable to type 'string[]'.` The `createAgentSession` call passes `tools: createReadOnlyTools(targetDir)` at `extensions/adversarial-review.ts:302`; current Pi types expect a different shape.
  - `extensions/question.ts(54,5)` and `(88,5): Expected 1 arguments, but got 2.` These are `Type.Optional(QuestionTypeSchema, { description })`; with `typebox`, put metadata elsewhere or use supported signature.
  - `extensions/todos.ts(796,40): number | undefined` to `number` in todo settings normalization.
  - `extensions/todos.ts(1842,26)` and `(1847,56): Property 'todo' does not exist on type 'TodoToolDetails'` after list/list-all narrowing.
  - `extensions/todos.ts(1966,33)` and `(2105,21): KeybindingsManager` not assignable to `KeybindingMatcher` due string vs `keyof Keybindings`.
  - `extensions/todos.ts(2151,26): Property 'requestRender' does not exist on type 'never'.`

## Risks and required fixes before shipping

1. **Typecheck is blocking.** The new target extensions introduce current TS failures. Fix before `npm run check` can pass.
2. **Lint is blocking.** `skills/art` and `interactive-artifacts/public/app.js` add lint diagnostics; existing Claude Code plugin lint failures also block global `npm run lint` unless fixed or excluded.
3. **`todos` mutates files without visible `withFileMutationQueue()`.** `rules/extensions.md:9` requires wrapping custom tools that mutate files. `extensions/todos.ts` writes/unlinks files around lines 839, 990, 1038, 1042, 1068, 1486 and registers mutating `todo` actions at `todos.ts:1516`; no `withFileMutationQueue` usage found.
4. **`question` uses `Type.Union([Type.Literal(...)])` for a string enum.** `extensions/question.ts:38-42` violates `rules/extensions.md:8`; should use `StringEnum` from `@mariozechner/pi-ai`.
5. **Global skill rule not yet satisfied.** Current `~/.pi/agent/skills/<target>` paths are real directories, not symlinks to repo canonical paths. Decide whether this package change should also update the global install state; that is an external mutation and should be explicitly approved.
6. **Art skill is large and contains legacy local-harness docs.** `skills/art/SKILL.md:24-33` tells Pi to ignore unavailable legacy tooling and `localhost:<port>` notifications, but the workflow files still contain those strings. This is intentional adaptation, but gitleaks/PII scan should run before staging.
7. **Secrets/placeholders.** No literal secret values found in a basic grep, but target resources mention env names and placeholders: `OPENAI_API_KEY`, `GOOGLE_API_KEY`, `REPLICATE_API_TOKEN`, `REMOVEBG_API_KEY`, `DISCORD_BOT_TOKEN`, `MIDJOURNEY_CHANNEL_ID`, `CONTEXT7_API_KEY`, Brave/Google/Tavily keys. Keep `.env` ignored; `skills/art/.env.example` is intentionally included.
8. **External API dependencies.** `web-tools` requires `BRAVE_SEARCH_API_KEY`, optional Google CSE credentials, and `TAVILY_API_KEY`; `find-docs` expects Context7 CLI; `art` tooling expects image-service credentials. Catalog/readme should not imply these work without user credentials.
9. **Pack size.** Dry pack is about 8.6 MB unpacked, mostly due art assets/workflows. Accept or prune intentionally.
10. **Import drift with global extensions.** If future copy overwrites local `typebox` imports in `adversarial-review.ts` or `interactive-artifacts/index.ts`, package may need `@sinclair/typebox` dependency or import normalization again.

## Suggested implementation order

1. Ensure the target copy is present using the copy plan above, preserving the `typebox` import normalization.
2. Add all target rows to `docs/catalog.md`.
3. Fix target extension rule/typecheck issues:
   - `question`: switch question type enum to `StringEnum`; fix `Type.Optional` metadata usage.
   - `todos`: add mutation queue or document why not applicable; fix narrowing/keybinding/rootTui type errors.
   - `adversarial-review`: update `createAgentSession` `tools` argument to current Pi API type.
4. Fix target lint issues; decide whether to also fix pre-existing out-of-scope lint blockers so `npm run check` passes.
5. Run validation commands above.
6. Only after validation and explicit approval, convert global target skill dirs to symlinks back to repo canonical dirs.
