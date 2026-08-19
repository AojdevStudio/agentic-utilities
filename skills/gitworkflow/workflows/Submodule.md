# Submodule Workflow

Structural submodule changes: add, update, remove, status, sync. Content changes inside dirty submodules belong to the Commit workflow (its Phase 1 commits submodules first, then the parent pointer).

## add `<url> [path]`

Path defaults to the repo name from the URL. Refuse when the path exists or is already a submodule. After `git submodule add` + `--init --recursive`: if the path was gitignored, offer (AskUserQuestion) to remove that ignore line — submodules must be tracked. Report the staged `.gitmodules` + path and point at the Commit workflow.

House path conventions: related repos → `repos/<name>`; shared libraries → `packages/<name>` or `libs/<name>`; docs → `docs/<name>`; tools → `tools/<name>`.

## update

Fetch all submodule remotes, show what's ahead, and ask before updating (`all / choose / cancel`). Then `git submodule update --remote --merge`, show the resulting status, and point at Commit for the pointer update.

## remove `<path>`

**Confirm with the user first** — this deletes the directory. Then the full three-step (partial removal leaves a haunted repo):

```bash
git submodule deinit -f <path>
rm -rf .git/modules/<path>
git rm -f <path>
```

## status

`git submodule status --recursive` plus a dirty-content sweep, rendered as a table (submodule / status / commit / dirty). Signals: space = clean at recorded commit, `+` = ahead (parent commit needed), `-` = uninitialized, `U` = merge conflict.

## sync

`git submodule sync --recursive` after `.gitmodules` URL edits; echo each submodule's resolved remote as verification.
