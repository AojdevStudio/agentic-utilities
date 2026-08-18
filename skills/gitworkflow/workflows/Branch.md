# Branch Workflow

Create and finish Git Flow branches with repo-aware base detection.

## Repo shape (detect first)

Integration branch = `develop` when the repo has one (local or remote), else the default branch. Never hard-code `develop` without confirming it exists.

| Type | Branch from | Merges to |
|------|-------------|-----------|
| `feature/<name>` | integration branch | PR to integration branch |
| `release/vX.Y.Z` | integration branch | default branch, then back-merge to develop *only when develop exists and differs* |
| `hotfix/<name>` | **default branch** | default branch, then the same conditional back-merge |

## Start

From an up-to-date base branch (no uncommitted changes, base current with origin), create the typed branch and push with tracking. Release branches also bump the version files and commit `🔖 release: bump version to X.Y.Z` before pushing.

## Finish

- **Feature:** the preferred path on shared repos is the **PullRequest** workflow targeting the integration branch; after merge, sync and delete the branch local + remote. Direct local merge only when the user explicitly wants a non-PR flow and the target isn't protected.
- **Release / hotfix:** merge `--no-ff` to the default branch, tag `vX.Y.Z` (annotated), push with tags, do the conditional develop back-merge, delete the branch local + remote. Hotfixes bump the patch version first.

**Complete when:** the work is on its target branch(es), the tag exists (release/hotfix), and the working branch is deleted both sides.

## Pre-finish checklist

No uncommitted changes · tests passing · no conflicts · remote current · correct target for this repo's shape · PR-template/issue-link requirements satisfied when using the PR path.
