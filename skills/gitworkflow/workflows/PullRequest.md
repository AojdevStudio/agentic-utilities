# Pull Request Workflow

Create PRs with repo-aware base detection and PR-template / issue-link compliance, then hand off to CIMerge.

## 1. Base branch and pre-checks

- Integration branch = `develop` when the repo has one (local or remote), else the default branch.
- `feature/*` targets the integration branch; `release/*` and `hotfix/*` target the default branch.
- Push the branch with tracking. Stop if the tree is dirty in a way that would make the PR misleading, the branch obviously targets the wrong base, or the repo requires issue linkage and none can be resolved (below).

## 2. Repo requirements

Read `.github/PULL_REQUEST_TEMPLATE.md` (either case) and any contributing docs for issue-link rules. When a template exists, mirror its sections and wording exactly. When the repo enforces an issue-link check, the body must contain either a real closing keyword (`Closes #123` / `Fixes #123` / `Resolves #123`) or the repo's exact no-issue marker checked (`- [x] No issue required …`). **Never leave placeholders** like `Closes #` or `Closes #123 (if applicable)`.

## 3. Issue link resolution (stop at the first hit)

1. User-supplied issue number.
2. Number embedded in the branch name.
3. Closing keyword already authored in a commit message on the branch.
4. `gh issue list --assignee @me` — offer the list for a human pick.
5. The repo's no-issue path, only when the template offers it AND this is genuinely docs/dependency/housekeeping.
6. Stop and ask — the repo requires an issue and nothing resolved one.

Validate any detected issue actually exists in this repo before using it.

## 4. Body and creation

Body sections: **Summary** (key changes), **Linked issues** (the resolved line from step 3), **Verification** (the concrete commands actually run — truthful and copy-pastable), screenshots when UI changed. Prefer `--body-file` over inline heredocs when templates matter.

## 5. Verify what GitHub parsed (required)

Immediately after creation, read back `baseRefName`, `body`, and `closingIssuesReferences` — do not assume GitHub parsed the body as intended. Wrong base → fix before anything else. Closing-keyword expected but `closingIssuesReferences` empty → the body is malformed; fix in place. No-issue path → confirm the box is `[x]`.

## 6. Sidebar metadata (required — filled or explicitly reported)

A PR isn't fully created while the sidebar sits empty. Source metadata from the linked issue when present, repo conventions otherwise:

- **Assignee:** the author/runner (`@me` on single-user repos).
- **Labels:** copy the issue's labels minus issue-only workflow controls (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`); add agent/automation labels only when they already exist in the repo.
- **Project:** the linked issue's own project membership is the source of truth; resolve owner and project number at runtime, never hardcode. Unresolvable → report `Project: none resolved`.
- **Milestone:** copy from the linked issue when set.
- **Reviewers:** only from explicit sources (user instruction, repo docs, CODEOWNERS — which routes review even when the sidebar looks empty). Never invent reviewers; none known → report `Reviewers: none configured`.

Finish with one read-back of number/labels/assignees/milestone/projects/reviewers/closingIssues as the evidence.

## 7. Continue to CIMerge (default: yes)

Creating the PR flows straight into `workflows/CIMerge.md` unless the user said "just create the PR" / "don't merge yet" — then stop and report:

```
## Pull Request Created
**Title:** … **Branch:** <head> → <base> **URL:** …
**Issue link:** <resolved line>
**Summary:** <commits, files, +/− stats>
**Next:** /GitWorkflow merge
```
