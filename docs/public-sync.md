# Public skill sync

Most skills under `skills/` are **generated** from a private skill store. You edit
the skill once, in the store, and this repo is rebuilt from it with private detail
removed. `public-manifest.json` is the contract; `scripts/sync-public.mjs` executes it.

Skills the manifest marks `public-owned` or `forked` are authored here and are never
touched by the sync.

## Two manifests, on purpose

`public-manifest.json` is tracked and holds only **structural** patterns: shapes that are
private regardless of who maintains the repo, like a macOS home path or an RFC1918 address.

`public-manifest.local.json` is **gitignored** and holds every pattern or replacement that
**names** something: your machines, private repos, email domains, internal framework paths.
An enumeration of those names is itself metadata about your private ecosystem, so shipping
the list publicly would leak the very thing the list exists to protect. This mirrors the
`.gitleaks.toml` / `.gitleaks.local.toml` split the repo already uses.

The sync merges them (overlay patterns appended, overlay replacements last) and **exits 1
if the overlay is missing**. A scrub that quietly covers less than intended is worse than
one that refuses to run. Copy `public-manifest.local.json.example` to get started.

## Commands

```bash
bun scripts/sync-public.mjs --check          # report drift, write nothing (exit 1 on drift)
bun scripts/sync-public.mjs                  # write
bun scripts/sync-public.mjs --prune          # also delete orphaned generated files
bun scripts/sync-public.mjs --skill find-docs
```

The store defaults to `~/.agents/skills`. Point `AGENT_SKILLS_STORE` elsewhere to
override it. When the store is absent, `--check` exits 0 with a notice saying drift
was **not** verified, so contributors without the store are not blocked. Only the
golden tests run everywhere.

## Modes

| Mode | Who is canonical | What sync does |
|---|---|---|
| `mirror` | the store | Copies and transforms store files into `skills/<name>/`. |
| `public-owned` | this repo | Skipped entirely. For skills authored here with no private counterpart. |
| `forked` | both, deliberately | Skipped, with a required `reason` recording why they diverged. |

`mirror` entries may declare:

- `source`: the store directory name, when it differs from the public one
  (`git-workflow` in the store, `gitworkflow` here).
- `publicOwned`: globs the sync must never write **or** delete. This is what lets a
  public skill ship tooling (`scripts/`, `fixtures/`, `agents/openai.yaml`) that the
  store does not carry.
- `exclude`: globs dropped from the published output entirely. For files that are
  private by nature rather than by wording.
- `replace` / `dropLines`: per-skill text rules, applied after the global ones.
  `dropLines` removes whole lines matching a regex, which is how a table row pointing
  at an excluded file gets removed.

An entry that is not `mirror` must record a `reason`. A test enforces that, so the
reasons stay readable as a record of why two copies diverged.

## The four gates

Being "verifiable" here means a transform gap fails a run rather than shipping.

1. **Leak assertion** (`scripts/sync-public.mjs`). Every transformed text file is
   scanned against `leakPatterns` *after* transforms. Any hit aborts that skill with a
   `file:line [pattern-id] excerpt` report and writes nothing. This is the backstop for
   a replacement rule that was never written, which is the failure mode a find-and-replace
   pipeline otherwise hides. Note that only extensions in `textExtensions` are scanned:
   anything else is copied byte-for-byte, so add an extension before publishing a new
   file type.

2. **Excluded-reference check.** Excluding a private file is fine. Leaving the published
   SKILL.md pointing at it ships a broken skill, so any published text that still names
   an excluded path aborts the run. Resolve it by publishing the target, or by dropping
   the pointer with `dropLines` / `replace`.

3. **Golden tests** (`scripts/sync-public.test.mjs`, part of `bun run test`). The
   transform chain and every leak pattern are tested against fixtures, so the scrub
   itself is proven rather than trusted. These run without the store.

4. **gitleaks pre-commit**. `.gitleaks.toml` (tracked, structural PII) plus
   `.gitleaks.local.toml` (gitignored, per-contributor terms). Independent of the sync,
   so it also covers hand-edited files.

Run `--check` on a schedule to catch staleness. Nothing in `bun run check` depends on
the store being present.

## Formatting owns nothing generated

`biome.json` excludes `skills/*/assets/**` and `skills/*/templates/**`. Those are payloads
a skill ships, and their formatting belongs to whoever wrote them. Without the exclusion,
`lint:fix` rewrites generated files and the very next `--check` reports drift that no one
introduced.

## Circular sources

A store entry that is a symlink **into this repo** makes source and destination the
same bytes, so "syncing" it means no scrub ever ran. The script detects this and
refuses:

```
diataxis-docs-site: store entry resolves inside this repo (…/skills/diataxis-docs-site).
  It is still a symlink into the public checkout, so no scrub can run.
  Materialize it in the store first, then re-run.
```

Fix it by replacing the store symlink with a real directory holding the content, then
repointing any harness lane symlinks at the store rather than at this repo.

## Adding a skill

1. Add an entry to `public-manifest.json` with a mode. Non-`mirror` modes require a `reason`.
2. `bun scripts/sync-public.mjs --skill <name> --check` and read the plan.
3. Resolve any leak findings by fixing the source, adding a transform, or excluding the file.
4. Sync, then run `npm test`. A skill with content tests will tell you if the store
   version and the published tooling have drifted apart.
