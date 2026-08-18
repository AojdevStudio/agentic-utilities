# CI Setup Workflow

Scaffold GitHub Actions for a repo: detect the stack, collect parameters via `AskUserQuestion`, render `.github/workflows/*.yml` from the bundled Handlebars templates, then hand to Commit → PullRequest → CIMerge so the new CI proves itself on its own bootstrap PR. This is the setup half; CIMerge owns monitor/merge.

## 0. Runner inventory

Before targeting self-hosted runners, confirm the labels exist (`gh api orgs/$ORG/actions/runners`, falling back to the repo-level endpoint). Required by the default templates — Linux: `homelab-ci`; macOS: `homelab-macos`, `mac-mini-m4` (GitHub auto-adds `self-hosted`, `Linux`, `X64`, `macOS`, `ARM64` — never re-declare those).

Labels missing → AskUserQuestion: register org runners now (recommended — generate a migration script) / use repo-level runners / all GitHub-hosted / stop. The migration script (written to `MEMORY/WORK/{slug}/runner-migration.sh`, **shown as a diff and run only on confirmation**) mints an org registration token, SSHes to the runner host (`ssh proxmox 'sudo pct exec 120 -- …'` for VM 120, `ssh macmini`), does `./svc.sh stop && ./svc.sh uninstall`, `./config.sh remove --token <T>`, re-registers against `https://github.com/$ORG`, restarts. The homelab `github-runner-{ci,macos}/` install scripts (via `Skill("homelab")`) work with org URLs unchanged.

## 1. Probe → confirm

Detect the stack from manifests and configs (runtime, package manager, frameworks, DB tooling, test runners, linters, existing workflows — never clobber those silently). Confirm the profile with the user before scaffolding.

## 2. Category menu (multiSelect)

PR gate · Secret scan (gitleaks) · DB safety (migration diff dry-run) · Worker safety (wrangler dry-run) · E2E (Playwright vs preview) · Native build (Tauri, macOS runner) · Deploy · GitHub Release with binaries (tag-triggered) · Release notes (tag-triggered, changelog-only) · Auto-release on merge · Dependabot · Dependabot auto-merge · Local hooks (Lefthook) · Scheduled checks.

**XOR guard:** "Auto-release on merge" is mutually exclusive with both tag-triggered Release categories — enforce before scaffolding, resolving via AskUserQuestion (auto-release for continuously-shipping apps / tag-triggered for deliberate cuts). Never write `release-auto.yml` alongside `release.yml`/`release-notes.yml`. Why: `workflows/Release.md` § Posture (the single source for the GITHUB_TOKEN/double-publish facts).

**Dependabot is on by default** — if unselected, offer it once (weekly, grouped minor+patch; the `github-actions` ecosystem is always included even in manifest-less repos).

## 3. Per-category parameters (AskUserQuestion, detected defaults first)

- **All categories:** runner — `[self-hosted, homelab-ci]` recommended for test/build/lint; **GitHub-hosted for anything touching secrets (deploy/release) — ephemeral isolation is the rule**; homelab-macos only for macOS-required jobs.
- **PR gate:** test/lint/build commands, runtime version (from `engines`/`.nvmrc`).
- **DB safety:** migration tool (from probe), shadow DB secret name (default `SHADOW_DATABASE_URL`).
- **Native build / Release-with-binaries:** targets matrix, code signing (`APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD` + `KEYCHAIN_PASSWORD` secrets), build command, artifacts glob (binary kind), draft?, opt-in self-hosted macOS (default no).
- **Release notes:** changelog path, draft?.
- **Auto-release:** initial version when untagged (default `v0.1.0`), changelog path. Bump rules are baked into the template (canonical: `templates/ci/release-auto.yml.hbs`); no question needed.
- **Dependabot:** ecosystems (pre-checked from probe; `github-actions` non-optional), schedule, group minor+patch?, offer auto-merge.
- **Auto-merge:** patch-only (default) or patch+minor; majors never auto-merge; fires only when `github.actor == 'dependabot[bot]'` and the PR gate passed.
- **Lefthook:** package manager, Rust/Python hook inclusion (auto from probe), pre-push test command. Lint/format run their **fixers** with `stage_fixed: true` (auto-fixable never blocks); typecheck and lockfile-drift are pure blockers.

## 4. Scaffold (safety-gated)

Per category: render the template, **show the YAML as a diff, write only on confirmation**, verify with `git diff` after.

- **Repo Variables:** after the first self-hosted-targeting workflow, offer to set `SELF_HOSTED_LINUX` / `SELF_HOSTED_MACOS` (JSON-array strings, org-level preferred so all repos inherit).
- **Lefthook:** after writing the config, run `lefthook install` once (`bunx lefthook install` fallback) — the only place CISetup touches `.git/hooks`; hooks fire on the *next* commit. Install failure is non-fatal: surface the manual command.
- **Fork safety (every self-hosted template):** paired jobs split on `if: github.event.pull_request.head.repo.full_name == github.repository` — same-repo PRs run self-hosted, fork PRs run `ubuntu-latest`. Persistent runners + untrusted fork code = credential theft; public repos should also require approval for first-time contributors.
- **Fallback policy:** `main` jobs read `SELF_HOSTED_LINUX_AVAILABLE`, which `runner-health-check.yml.hbs` (scheduled, must run hosted) flips false when the runner is offline >5 min.

## 5. Verify and self-test

`actionlint` + `yamllint` when installed, `gh workflow list` to confirm GitHub sees them. Then Commit → PullRequest → CIMerge: the bootstrap PR exercises the new CI, and failures flow into CIMerge's fix loop.

## Templates inventory (`templates/ci/`)

| Template | Purpose | Runner | Trigger |
|----------|---------|--------|---------|
| `node-pr-gate.yml.hbs` | typecheck+lint+test+build smoke | homelab-ci | PR + push |
| `gitleaks.yml.hbs` | secret scan | homelab-ci | PR + push |
| `drizzle-migrate-diff.yml.hbs` / `prisma-migrate-diff.yml.hbs` | migration dry-run diff | homelab-ci | PR touching schema |
| `playwright-e2e.yml.hbs` | e2e vs preview URL | homelab-ci | PR after preview |
| `tauri-macos-build.yml.hbs` | macOS arm64 build smoke | homelab-macos | PR + tag |
| `wrangler-deploy-dry-run.yml.hbs` | workers dry-run | homelab-ci | PR touching workers |
| `release.yml.hbs` | build matrix → Release with binaries, body from CHANGELOG | hosted | tag + dispatch |
| `release-notes.yml.hbs` | changelog-only Release | hosted | tag `v*` + dispatch |
| `release-auto.yml.hbs` | bump→changelog→tag→self-publish (XOR with the two above) | hosted | push to main |
| `dependabot.yml.hbs` | version+security updates config | n/a | Dependabot service |
| `dependabot-auto-merge.yml.hbs` | auto-merge after CI, patch-default | hosted | Dependabot PRs |
| `lefthook.yml.hbs` | local hooks: fixers + blockers | n/a | commit/push |
| `runner-health-check.yml.hbs` | flips the AVAILABLE var | hosted (must be) | cron 5 min |

New template = drop the `.hbs` file, add a row here and a Phase-3 question; the menu picks it up next invocation.

## Runner conventions (house policy, 2026-05-19)

| Host | Labels | Notes |
|------|--------|-------|
| Proxmox VM 120 (org) | `self-hosted, homelab-ci` | |
| Mac Mini M4 (org) | `self-hosted, homelab-macos, mac-mini-m4` | `tauri-macos` is a Finance-Guru-v2 repo-level override only |

Variables: `SELF_HOSTED_LINUX` / `SELF_HOSTED_MACOS` (JSON arrays) and `SELF_HOSTED_LINUX_AVAILABLE` (string bool), org-level. **Labels are referenced in YAML only via `${{ vars.* }}`** — hardcoding leaks topology and breaks portability.

## Gotchas (verified — CI-domain single source)

- **Org-runner registration needs org-scope auth** (`gh auth status` must show it) or fails silently; repo-level needs only repo scope.
- **`fromJSON(vars.X)` with an undefined variable errors at *run* time, not parse time** — always fall back: `${{ fromJSON(vars.SELF_HOSTED_LINUX || '["ubuntu-latest"]') }}`.
- **Re-registering a per-repo runner at org level** requires clean uninstall first (`./svc.sh uninstall && ./config.sh remove`) or a full reinstall — the install dir's `.runner` file keeps the old repo association.
- **Actions cache is per-repo even on org runners** — sibling repos share nothing; use explicit `actions/cache` keys.
