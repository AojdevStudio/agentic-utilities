# Deploy GitHub Actions Workflow

Deploy a `.github/workflows/` file (or related config) to the default branch from an isolated branch, without merging an unrelated feature branch.

## When to Use

- The user is on a feature branch with unrelated work and wants a workflow live on `main` *now*
- A workflow needs to be merged before the feature branch it was created on is ready
- Any situation where GitHub Actions files must land on the default branch independently

## Variables

```bash
WORKFLOW_FILES:   # space-separated paths under .github/ (e.g., ".github/workflows/foo.yml .github/labeler-config.json")
SOURCE_COMMIT:    # commit hash on the current branch that contains the workflow files (optional)
ISOLATED_BRANCH:  # name for the new branch (e.g., "feat/deploy-issue-labeler")
```

## Workflow

### Phase 1: Prepare

Save the user's current branch and working state, then create a clean isolated branch from `origin/defaultBranch`:

```bash
ORIGINAL_BRANCH=$(git branch --show-current)

# Stash any uncommitted changes so we can switch branches safely
git stash push -m "wip: deploy-workflow stash"

# Create isolated branch from the latest remote default branch
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name' 2>/dev/null || git remote show origin | sed -n '/HEAD branch/s/.*: //p')
git fetch origin "$DEFAULT_BRANCH"
git checkout -b "$ISOLATED_BRANCH" "origin/$DEFAULT_BRANCH"
```

### Phase 2: Extract Workflow Files

#### Option A — Cherry-pick from existing commit

If the workflow files already exist in a commit on the feature branch:

```bash
git cherry-pick "$SOURCE_COMMIT" --no-commit
```

#### Option B — Copy from working tree

If the files are in the working tree but not yet committed:

```bash
# Copy each file from the stashed/original working tree
git checkout "$ORIGINAL_BRANCH" -- $WORKFLOW_FILES
```

### Phase 3: Clean

Remove any unrelated files that came along (e.g., from a messy cherry-pick or branch state):

```bash
# Unstage anything not in WORKFLOW_FILES
git reset HEAD
# Re-stage only the workflow files
git add $WORKFLOW_FILES
# Discard everything else
git checkout -- .
git clean -fd
```

Verify the commit contains *only* workflow files:

```bash
git diff --cached --name-only
```

### Phase 4: Commit and Push

```bash
git commit -m "feat(ci): deploy WORKFLOW_NAME

- Deployed from isolated branch to avoid merging unrelated feature work"
git push -u origin "$ISOLATED_BRANCH"
```

### Phase 5: Merge to Default Branch

Open a PR and wait for automated review feedback before merging. Even config-only changes can receive actionable review comments (e.g., stale label handling, YAML syntax, permission scopes).

```bash
gh pr create \
  --base "$DEFAULT_BRANCH" \
  --head "$ISOLATED_BRANCH" \
  --title "feat(ci): deploy WORKFLOW_NAME" \
  --body "Isolated deployment of GitHub Actions workflow."

PR_NUMBER=$(gh pr list --head "$ISOLATED_BRANCH" --json number -q '.[0].number')
```

**Wait for reviews:**

```bash
sleep 240
```

> **Mandatory minimum:** 240 seconds. Automated reviewers (Codex, CodeRabbit, GitGuardian) often post 60–180 seconds after the PR is opened.

**Check for review feedback before merging:**

```bash
REVIEW_DECISION=$(gh pr view "$PR_NUMBER" --json reviewDecision -q '.reviewDecision')
COMMENTS=$(gh pr view "$PR_NUMBER" --json reviews --jq '.reviews[] | select(.state == "COMMENTED" or .state == "CHANGES_REQUESTED") | {author: .author.login, state: .state}')

echo "Review decision: $REVIEW_DECISION"
echo "Comments: $COMMENTS"
```

If `CHANGES_REQUESTED` or substantive `COMMENTED` reviews exist, address them before merging. Otherwise:

```bash
gh pr merge "$PR_NUMBER" --squash --delete-branch
```

### Phase 6: Restore User State

Return to the original branch and restore working tree:

```bash
git checkout "$ORIGINAL_BRANCH"
git stash pop
```

---

## Validation Rules

- [ ] Only files under `.github/` are in the final commit
- [ ] The isolated branch is based on `origin/$DEFAULT_BRANCH`, not the feature branch
- [ ] Uncommitted changes on the original branch are preserved via stash
- [ ] The workflow file syntax is valid (optional: run `actionlint` if available)

## Error Handling

| Error | Action |
|-------|--------|
| Cherry-pick includes unrelated files | Reset, then `git checkout ORIGINAL_BRANCH -- $WORKFLOW_FILES` |
| Stash pop conflicts | Resolve manually; the stash remains available as `stash@{0}` |
| PR merge blocked by branch protection | Report PR URL and stop; user must merge manually |
| Workflow file has YAML syntax errors | Run `actionlint` or push a fix commit to the isolated branch |
