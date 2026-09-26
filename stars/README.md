# stars

`stars` verifies exact GitHub repository identities, stars requested repositories, and keeps a local review ledger. It supports batch requests and reports each result separately. A successful star remains reported as `starred` if the subsequent ledger sync fails.

## Install

Requires Bun 1.2 or newer and GitHub CLI (`gh`). Supported on macOS and Linux. Authenticate with `gh auth login`, then check the active account with `gh api user --jq .login`. The authenticated account needs permission to manage its own stars. No token is stored by `stars`.

```bash
bun add --global aoj-stars
stars --version
stars --help
```

If `stars` resolves to an older executable, inspect `type -a stars` and put Bun's global bin directory (`bun pm bin -g`) first on `PATH`. Do not remove an existing installation before preserving its ledger.

## Upgrade

Install a specific released version with `bun add --global aoj-stars@<version>`. Then check `type -a stars`, `stars --version`, and `stars status --json` to confirm the active executable, version, and ledger account. Keep `STARS_DATA_DIR` set to the prior state directory until its ledger and review queue have been migrated.

## Use

```bash
stars star https://github.com/OWNER/REPO --json
stars star OWNER/ONE OWNER/TWO --json
stars sync --json
stars status
stars queue --json
stars review -n 5
stars decide OWNER/REPO install --note "Evaluate for my workflow"
stars actions --json
stars done OWNER/REPO
```

`star` accepts exact GitHub repository URLs and `OWNER/REPO` references. It resolves the canonical repository through GitHub before writing a star. A mention in a video, page, or conversation is a candidate to verify, not a request to star. Agents should run `star` only after the user asks for that action. Repositories already starred are reported as `already-starred`. Results include each canonical URL and workflow status; a mixed batch exits nonzero if any item or sync fails.

`unstar` interactively asks before removing each repository marked by a review decision.

## State and configuration

The ledger and generated review queue live in `${XDG_STATE_HOME}/stars/`, or `${HOME}/.local/state/stars/` when `XDG_STATE_HOME` is unset. Set `STARS_DATA_DIR` to keep a prior ledger in place or choose another private directory. Never place a personal ledger in a public repository. The ledger belongs to one GitHub account; `stars` refuses to use it when `gh` is authenticated as another account.

Optional evidence sources improve review priority and annotate records. Core starring, sync, review, and follow-up commands work without them.

| Variable | Meaning |
| --- | --- |
| `STARS_DATA_DIR` | Directory for `ledger.json` and `review.md` |
| `STARS_PROJECTS_PATH` | Markdown project inventory in the existing active-project table format |
| `STARS_CHECKOUT_ROOTS` | Local checkout roots, separated by the platform path delimiter (`:` on macOS and Linux) |
| `STARS_SKILLS_LOCK_PATH` | Optional skills lock JSON used as repository evidence |
| `STARS_CODEX_CONFIG` | Codex config path for enabled plugin evidence; defaults to `${HOME}/.codex/config.toml` |
| `STARS_SSH_HOST` and `STARS_SSH_ROOT` | Optional SSH checkout inventory; set both or neither |

For example:

```bash
export STARS_DATA_DIR="$HOME/.local/state/stars"
export STARS_CHECKOUT_ROOTS="$HOME/Projects"
stars sync
```

For a previous installation, point `STARS_DATA_DIR` to its existing state directory first, verify `stars status --json`, then move that directory to the new default if desired. Preserve `ledger.json` and `review.md` together and keep `STARS_DATA_DIR` set until the move is complete.

## Troubleshooting

- `gh` errors: run `gh auth status` and `gh api user --jq .login` to inspect authentication.
- Account mismatch: restore the matching `gh` account or choose a separate `STARS_DATA_DIR`; do not overwrite a different account's ledger.
- Workflow failure after a star: the JSON result still reports `starred`; fix the named sync error and run `stars sync`.
- Optional SSH inventory failure: unset `STARS_SSH_HOST` and `STARS_SSH_ROOT` or repair that host's access.

## Development

```bash
cd stars
bun test
bun run pack:dry
bun run pack
```

The published package includes only the CLI source, this README, and the license. It has no dependency on the root Pi package, the source checkout, or private machine files.
