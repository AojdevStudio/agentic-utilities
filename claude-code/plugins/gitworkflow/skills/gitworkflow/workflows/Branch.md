# Branch Workflow

Create and manage feature, release, and hotfix branches with repo-aware base branch detection.

## Variables

```bash
BRANCH_TYPE: {{feature|release|hotfix}}
BRANCH_NAME: $ARGUMENTS
ACTION: {{start|finish}}
DEFAULT_BRANCH: detected from GitHub / origin HEAD
INTEGRATION_BRANCH: develop when present, otherwise DEFAULT_BRANCH
```

Detect repo branches first:

```bash
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name' 2>/dev/null || git remote show origin | sed -n '/HEAD branch/s/.*: //p')
if git show-ref --verify --quiet refs/heads/develop || git ls-remote --exit-code --heads origin develop >/dev/null 2>&1; then
  INTEGRATION_BRANCH=develop
else
  INTEGRATION_BRANCH="$DEFAULT_BRANCH"
fi
```

## Workflow

### Start a Branch

#### Feature Branch (from `INTEGRATION_BRANCH`)

```bash
# Step 1: Ensure the integration branch is up to date
git checkout "$INTEGRATION_BRANCH"
git pull origin "$INTEGRATION_BRANCH"

# Step 2: Create and switch to feature branch
git checkout -b feature/BRANCH_NAME

# Step 3: Push to remote and set up tracking
git push -u origin feature/BRANCH_NAME
```

**Pre-creation validation:**
- [ ] Check no uncommitted changes: `git status`
- [ ] Verify `INTEGRATION_BRANCH` is current: `git log origin/$INTEGRATION_BRANCH..$INTEGRATION_BRANCH` (should be empty)
- [ ] Confirm the repo uses `develop` before hard-coding `develop`

#### Release Branch (from `INTEGRATION_BRANCH`)

```bash
# Step 1: Create release branch from integration branch
git checkout "$INTEGRATION_BRANCH"
git pull origin "$INTEGRATION_BRANCH"
git checkout -b release/vX.Y.Z

# Step 2: Update version numbers
# Edit package.json, version files, etc.

# Step 3: Commit version bump
git commit -am "🔖 release: bump version to X.Y.Z"

# Step 4: Push release branch
git push -u origin release/vX.Y.Z
```

#### Hotfix Branch (from `DEFAULT_BRANCH`)

```bash
# Step 1: Branch from repo default branch
git checkout "$DEFAULT_BRANCH"
git pull origin "$DEFAULT_BRANCH"
git checkout -b hotfix/BRANCH_NAME

# Step 2: Push to remote
git push -u origin hotfix/BRANCH_NAME
```

**Hotfix urgency indicators:**
- 🚨 Site down / Service unavailable
- 🔐 Security vulnerability discovered
- 💥 Data corruption or loss
- ⚠️ Critical feature broken in production

---

### Finish a Branch

#### Finish Feature

Preferred path for shared repos:

1. Run the **PullRequest** workflow and target `INTEGRATION_BRANCH`.
2. After the PR merges, sync and clean up:

```bash
git checkout "$INTEGRATION_BRANCH"
git pull origin "$INTEGRATION_BRANCH"
git branch -d feature/BRANCH_NAME
git push origin --delete feature/BRANCH_NAME
```

Only do a direct local merge when the user explicitly wants a non-PR flow and the target branch is not protected.

#### Finish Release

```bash
# Step 1: Merge to default branch
git checkout "$DEFAULT_BRANCH"
git pull origin "$DEFAULT_BRANCH"
git merge --no-ff release/vX.Y.Z

# Step 2: Tag the release
git tag -a vX.Y.Z -m "Release vX.Y.Z"

# Step 3: Push default branch with tags
git push origin "$DEFAULT_BRANCH" --tags

# Step 4: Back-merge to develop only when develop exists and differs from default
if [ "$INTEGRATION_BRANCH" != "$DEFAULT_BRANCH" ]; then
  git checkout "$INTEGRATION_BRANCH"
  git pull origin "$INTEGRATION_BRANCH"
  git merge --no-ff release/vX.Y.Z
  git push origin "$INTEGRATION_BRANCH"
fi

# Step 5: Clean up release branch
git branch -d release/vX.Y.Z
git push origin --delete release/vX.Y.Z
```

#### Finish Hotfix

```bash
# Step 1: Bump patch version
git checkout hotfix/BRANCH_NAME
# Update version (e.g., 1.2.0 → 1.2.1)
git commit -am "🔖 hotfix: bump version to X.Y.Z"

# Step 2: Merge to default branch
git checkout "$DEFAULT_BRANCH"
git pull origin "$DEFAULT_BRANCH"
git merge --no-ff hotfix/BRANCH_NAME

# Step 3: Tag the hotfix
git tag -a vX.Y.Z -m "Hotfix vX.Y.Z - BRANCH_NAME"

# Step 4: Push default branch with tags
git push origin "$DEFAULT_BRANCH" --tags

# Step 5: Back-merge to develop only when develop exists and differs from default
if [ "$INTEGRATION_BRANCH" != "$DEFAULT_BRANCH" ]; then
  git checkout "$INTEGRATION_BRANCH"
  git pull origin "$INTEGRATION_BRANCH"
  git merge --no-ff hotfix/BRANCH_NAME
  git push origin "$INTEGRATION_BRANCH"
fi

# Step 6: Clean up
git branch -d hotfix/BRANCH_NAME
git push origin --delete hotfix/BRANCH_NAME
```

---

## Branch Validation Rules

**Valid branch names:**
- ✅ `feature/user-authentication`
- ✅ `release/v1.2.0`
- ✅ `hotfix/security-patch`

**Invalid branch names:**
- ❌ `my-new-feature` (no prefix)
- ❌ `fix-bug` (wrong prefix for this workflow)

**Branch sources:**
- Features → branch from `INTEGRATION_BRANCH`
- Releases → branch from `INTEGRATION_BRANCH`
- Hotfixes → branch from `DEFAULT_BRANCH`

**Merge targets:**
- Features → PR to `INTEGRATION_BRANCH`
- Releases → merge/PR to `DEFAULT_BRANCH`, then back-merge to `develop` only when it exists
- Hotfixes → merge/PR to `DEFAULT_BRANCH`, then back-merge to `develop` only when it exists

---

## Pre-Merge Checklist

Before finishing any branch:
- [ ] No uncommitted changes
- [ ] Tests passing
- [ ] No merge conflicts
- [ ] Remote is up to date
- [ ] Correct target branch for this repo shape
- [ ] If using a PR workflow, issue-link / PR-template requirements are satisfied
