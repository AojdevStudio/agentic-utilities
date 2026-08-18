# Deploy GitHub Actions Workflow

Land `.github/` files on the default branch from an isolated branch, without dragging unrelated feature work along. Use when a workflow must go live before the feature branch it was authored on is ready.

## Invariants (the whole point)

- The isolated branch is cut from **`origin/<default-branch>`**, never from the feature branch.
- The final commit contains **only** the named `.github/` files — verify with `git diff --cached --name-only` before committing.
- The original branch's uncommitted work is preserved (stash before switching, pop after) and the checkout returns to it when done.
- The PR **waits for automated reviews to settle** before merging — even config-only changes draw actionable feedback (label handling, YAML syntax, permission scopes). Reviewers post 60–180s after the PR opens; poll checks to completion rather than one long sleep, per the CIMerge settle rules.

## Flow

1. Stash, branch from the fetched remote default branch.
2. Bring the files over — cherry-pick `--no-commit` from the source commit, or checkout the paths from the original branch — then strip anything unrelated that rode along until only the target files are staged.
3. Commit (`feat(ci): deploy <workflow> — isolated from feature work`), push, open the PR against the default branch.
4. Settle, address feedback, squash-merge with branch delete (branch protection blocks → report the URL and stop).
5. Restore: back to the original branch, pop the stash (conflicts → resolve; the stash survives as `stash@{0}`).

**Complete when:** the workflow file is on the default branch, the checkout is back on the original branch with its uncommitted work restored, and nothing else changed.
