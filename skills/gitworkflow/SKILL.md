---
name: gitworkflow
emoji: 🌿
description: "Smart Git workflow — Git Flow branching, CI monitoring, auto-merge, submodule awareness, issue analysis/routing, and deploy-workflow isolation. USE WHEN commit, branch, pull request, monitor CI, merge PR, create release, submodules, CodeRabbit, push and merge, --issue-analysis, analyze issues, route issues across worktrees, deploy workflow."
when_to_use: "USE WHEN commit, branch, pull request, monitor CI, merge PR, create release, submodules, CodeRabbit, push and merge, --issue-analysis, analyze issues, label issues, route issues across worktrees, plan beta cut, plan mvp cut, who should work on what, agent ownership labels, deploy workflow."
category: development
triggers:
  - commit
  - git
  - branch
  - pull request
  - release
  - merge
  - submodule
  - CI
  - checks
  - push and merge
  - monitor CI
  - auto-merge
  - wait for CI
  - is CI passing
  - merge my PR
  - submit PR
  - code review
  - --issue-analysis
  - analyze issues
  - label issues
  - route issues across worktrees
  - plan beta cut
  - plan mvp cut
  - who should work on what
  - agent ownership labels
  - deploy workflow
  - push workflow to main
  - merge workflow only
  - deploy gh action
context: fork
---

# GitWorkflow

Smart Git workflow engine with submodule detection, hook-aware commit strategies, **repo-aware branch targeting**, **CI monitoring & auto-merge**, **issue analysis/routing**, **deploy-workflow isolation**, and **changelog awareness**.

## Changelog Tool (GATED)

Every commit and release workflow uses the `changelog` CLI tool (`~/.local/bin/changelog`) for repo awareness.

```bash
changelog --auto --dry-run     # Preview unreleased changes (used after every commit)
changelog VERSION --auto       # Generate CHANGELOG.md entry (used in releases)
changelog VERSION --auto --force  # Non-interactive changelog generation
```

**Source:** `~/Projects/desktop-commander/scripts/changelog/`

This is **mandatory** — the user needs visibility into what's changing across repos. There is no skip flag. A gate with an escape hatch isn't a gate.

## Repository Adaptation Rules

Before running Branch, PullRequest, CIMerge, or Release workflows, detect the repo shape:

```bash
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name' 2>/dev/null || git remote show origin | sed -n '/HEAD branch/s/.*: //p')
if git show-ref --verify --quiet refs/heads/develop || git ls-remote --exit-code --heads origin develop >/dev/null 2>&1; then
  INTEGRATION_BRANCH=develop
else
  INTEGRATION_BRANCH="$DEFAULT_BRANCH"
fi
```

Rules:

- If `develop` exists, use Git Flow normally.
- If `develop` does **not** exist, treat the repo `defaultBranch` as the integration branch too.
- Feature branches PR into `INTEGRATION_BRANCH`.
- Release and hotfix branches PR into `DEFAULT_BRANCH`.
- If the repo has `.github/PULL_REQUEST_TEMPLATE.md`, use it as the starting shape for the PR body.
- If the repo has an issue-link check or contributing docs that require issue closure keywords, the PR body must contain a **real** closing reference like `Closes #123`, `Fixes #123`, or `Resolves #123`, or explicitly mark `No issue required` when the repo allows that path.
- Never leave placeholders like `Closes #`, `#ISSUE_NUM`, or a bare `#123` in prose and assume GitHub will auto-close the issue.

This matters for repos like Keepfolio, which now fail PR checks unless the PR body contains a valid closing keyword or an explicit `No issue required` marker.

## Workflow Routing

**When executing a workflow, output this notification directly:**

```
Running the **WorkflowName** workflow from the **GitWorkflow** skill...
```

| Workflow | Trigger | File |
|----------|---------|------|
| **Commit** | "commit", "commit changes", "make a commit" | `workflows/Commit.md` |
| **Branch** | "create branch", "start feature", "finish branch" | `workflows/Branch.md` |
| **Release** | "create release", "bump version", "tag release" | `workflows/Release.md` |
| **PullRequest** | "create PR", "open pull request", "submit PR" | `workflows/PullRequest.md` |
| **CIMerge** | "merge PR", "check CI", "wait for checks", "is CI passing", "monitor CI", "auto-merge" | `workflows/CIMerge.md` |
| **Submodule** | "add submodule", "add repo as submodule", "submodule add/update/remove" | `workflows/Submodule.md` |
| **IssueAnalysis** | `--issue-analysis`, "analyze issues", "label issues", "route issues across worktrees", "plan the cut", "who should work on what" | `workflows/IssueAnalysis.md` |
| **DeployWorkflow** | "deploy workflow", "push workflow to main", "merge workflow only", "deploy gh action" | `workflows/DeployWorkflow.md` |

## Examples

**Example 1: Smart commit with submodule handling**

```
User: "commit my changes"
→ Invokes Commit workflow
→ Detects dirty submodules, commits them first
→ Analyzes hooks to choose strategy (PARALLEL/COORDINATED/HYBRID)
→ Runs pre-commit validation
→ Generates conventional commit message with emoji
→ Executes commit
```

**Example 2: Create and manage a feature branch**

```
User: "create a feature branch for user authentication"
→ Invokes Branch workflow
→ Detects repo default/integration branch
→ Creates feature/user-authentication branch from develop when present, otherwise from the repo default branch
→ Pushes to remote with tracking
```

**Example 3: Create PR, monitor CI, and auto-merge**

```
User: "create a PR and merge it when CI passes"
→ Invokes PullRequest workflow
→ Detects target branch from repo shape + current branch type
→ Reads PR template / issue-link requirements when present
→ Creates PR with a valid closing keyword or marks no-issue-required when allowed
→ Continues to CIMerge workflow
→ Polls CI checks (GitHub Actions, CodeRabbit, GitGuardian, repo-specific metadata checks)
→ Waits 240s for automated reviews to settle (minimum 4 minutes)
→ Checks review decision (changes requested? approved? none?)
→ Auto-merges with squash when all clear
```

**Example 4: Create a release with version bump**

```
User: "create a release for version 2.0.0"
→ Invokes Release workflow
→ Analyzes commits to confirm MAJOR bump is appropriate
→ Creates release/v2.0.0 branch from develop when present, otherwise from the default branch
→ Updates version files
→ Generates changelog from conventional commits
→ Pushes release branch for testing
```

**Example 5: Route issues across coding-agent worktrees toward a beta cut**

```
User: "/git-workflow --issue-analysis --apply"
→ Invokes IssueAnalysis workflow
→ Detects worktrees (claude / codex / pi) and in-flight PRs
→ Asks for the cut sentence (north-star check)
→ Splits backlog into <cut>-blocker vs post-<cut>
→ Assigns issues to agents by warm context + file boundary
→ Creates labels: agent:<name>, <cut>-blocker, post-<cut>
→ Edits all open issues with appropriate labels
→ Prints routing table + ASCII route map + per-agent gh cheat-line
```

**Example 6: Deploy a GitHub Actions workflow from a feature branch without merging the feature**

```
User: "deploy this workflow to main, but don't merge my feature branch yet"
→ Invokes DeployWorkflow workflow
→ Stashes uncommitted changes on the current feature branch
→ Creates an isolated branch from origin/main
→ Cherry-picks or checks out only the workflow files
→ Commits, pushes, opens PR, merges with squash
→ Returns to the feature branch and restores the stash
→ Workflow is live on main; feature branch remains untouched
```

---

## Quick Reference: Commit Messages

Use Conventional Commits with emoji prefixes:

```
<emoji> <type>(<scope>): <description>

[optional body]

Co-Authored-By: AOJDevStudio
```

**Common Types:**

- ✨ `feat` - New feature
- 🐛 `fix` - Bug fix
- 📝 `docs` - Documentation
- 💄 `style` - Formatting/style
- ♻️ `refactor` - Code refactoring
- ✅ `test` - Tests
- 🔧 `chore` - Tooling, configuration

**Reference:** See `${PAI_DIR}/ai-docs/templates/emoji-commit-ref.yaml` for 50+ emoji mappings

---

## Quick Reference: Branch Commands

### Detect Base Branches

```bash
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name' 2>/dev/null || git remote show origin | sed -n '/HEAD branch/s/.*: //p')
if git show-ref --verify --quiet refs/heads/develop || git ls-remote --exit-code --heads origin develop >/dev/null 2>&1; then
  INTEGRATION_BRANCH=develop
else
  INTEGRATION_BRANCH="$DEFAULT_BRANCH"
fi
```

### Feature Branch

```bash
# Start
git checkout "$INTEGRATION_BRANCH" && git pull origin "$INTEGRATION_BRANCH"
git checkout -b feature/descriptive-name
git push -u origin feature/descriptive-name

# Finish (preferred: open a PR into $INTEGRATION_BRANCH)
```

### Release Branch

```bash
# Start
git checkout "$INTEGRATION_BRANCH" && git checkout -b release/vX.Y.Z
git commit -am "🔖 release: bump version to X.Y.Z"
git push -u origin release/vX.Y.Z

# Finish (merge to default branch, then back-merge to develop only when develop exists)
```

### Hotfix Branch

```bash
# Start (from repo default branch)
git checkout "$DEFAULT_BRANCH" && git checkout -b hotfix/descriptive-name
git push -u origin hotfix/descriptive-name

# Finish (merge to default branch, then back-merge to develop only when develop exists)
```

---

## Commit Strategy Detection

The Commit workflow automatically detects the optimal strategy based on pre-commit hooks:

| Hook Configuration | Strategy | Behavior |
|--------------------|----------|----------|
| No formatting hooks | PARALLEL | Stage multiple commits independently |
| Formatting hooks (non-aggressive) | COORDINATED | Stage and commit sequentially |
| Aggressive formatting (prettier --write) | HYBRID | Stage all, let hook format, single commit |

---

## Gotchas

- `lefthook install` in postinstall crashes on Vercel (not a git repo). Fix: `"postinstall": "lefthook install || true"`.
- Always check postinstall scripts before first Vercel deploy.
- Always verify `NEXT_PUBLIC_SITE_URL` env var is set to the production URL, not localhost, on first deploy.
- Do not assume every repo has `develop`. Detect it.
- Do not assume every PR can omit issue metadata. Inspect `.github/` and contributing docs first.

## Reusable Workflow Templates

Ready-to-copy GitHub Actions workflows stored in `templates/`:

| Template | Purpose | Files |
|----------|---------|-------|
| **Issue Auto-Labeler** | Deterministic keyword-based issue labeling, AI-swap-in ready | `templates/issue-labeler.yml` + `templates/labeler-config.json` |

Drop both files into a repo's `.github/` directory, customize `labeler-config.json` to match the repo's labels, and the workflow is live on the next push to the default branch.

## Supplementary Resources

**Detailed workflows, conflict resolution, error handling:**
Read: `AGENT.md`

**Comprehensive emoji commit reference:**
Read: `${PAI_DIR}/ai-docs/templates/emoji-commit-ref.yaml`
