# Git Workflow - Comprehensive Guide

This document provides detailed methodology for Git Flow branch management, conventional commits with emojis, and pull request creation.

**Quick Reference:** See `SKILL.md` for command cheatsheet.

## Repository Adaptation Overrides

These overrides take precedence over legacy Git Flow examples below:

- Detect the repo `defaultBranch` before branching or opening a PR.
- If `develop` exists, use it as the integration branch.
- If `develop` does **not** exist, treat the repo `defaultBranch` as the integration branch too.
- When a repo has `.github/PULL_REQUEST_TEMPLATE.md` or a PR metadata check, PR bodies must include a real closing keyword like `Closes #123` / `Fixes #123` / `Resolves #123`, or the repo's exact `No issue required` marker when allowed.
- Never leave placeholders like `Closes #`, `#ISSUE_NUM`, or a bare issue number and expect auto-close behavior.

For current operational behavior, prefer the workflow files in `workflows/` over older static examples in this guide.

---

## Table of Contents

1. [Git Flow Overview](#git-flow-overview)
2. [Branch Lifecycle Management](#branch-lifecycle-management)
3. [Commit Message Standards](#commit-message-standards)
4. [Release Management](#release-management)
5. [Conflict Resolution](#conflict-resolution)
6. [Error Handling](#error-handling)
7. [Pull Request Workflows](#pull-request-workflows)
8. [Best Practices](#best-practices)

---

## Git Flow Overview

### Branch Hierarchy

Git Flow maintains a structured branching model with distinct purposes:

```
main (production)
├── hotfix/* → merges back to main AND develop
│
develop (integration)
├── feature/* → merges back to develop
├── release/* → merges to main AND develop
```

**Protected Branches:**

- `main`: Production-ready code only. Never commit directly.
- `develop`: Integration branch for features. Never commit directly.

**Working Branches:**

- `feature/`*: New functionality (short-lived)
- `release/`*: Release preparation (short-lived)
- `hotfix/`*: Emergency production fixes (very short-lived)

### When to Use Each Branch Type

**Feature Branch** (`feature/descriptive-name`):

- Adding new functionality
- Non-urgent improvements
- Experimental work that can be tested in develop
- Lifespan: Days to weeks

**Release Branch** (`release/vX.Y.Z`):

- Preparing for production deployment
- Version bumping and final testing
- Release notes and documentation updates
- Lifespan: Hours to days

**Hotfix Branch** (`hotfix/descriptive-name`):

- Critical production bugs
- Security vulnerabilities
- Data corruption issues
- Lifespan: Minutes to hours

---

## Branch Lifecycle Management

### Feature Branch Complete Workflow

#### 1. Starting a Feature

```bash
# Step 1: Ensure develop is up to date
git checkout develop
git pull origin develop

# Step 2: Create and switch to feature branch
git checkout -b feature/user-profile-page

# Step 3: Push to remote and set up tracking
git push -u origin feature/user-profile-page
```

**Pre-creation validation:**

- Confirm you're on `develop` before branching
- Check no uncommitted changes: `git status`
- Verify develop is current: `git log origin/develop..develop` (should be empty)

#### 2. Working on a Feature

```bash
# Regular work cycle
git add <files>
git commit -m "$(cat <<'EOF'
✨ feat(profile): add user avatar upload

Implemented image upload with validation and S3 storage.

Co-Authored-By: AOJDevStudio
EOF
)"

# Push changes regularly
git push origin feature/user-profile-page
```

**During development:**

- Commit frequently with meaningful messages
- Keep commits atomic (one logical change per commit)
- Sync with develop periodically if feature takes more than a few days:
  ```bash
  git checkout develop && git pull origin develop
  git checkout feature/user-profile-page
  git merge develop
  ```

#### 3. Finishing a Feature

```bash
# Step 1: Final sync with develop
git checkout develop
git pull origin develop

# Step 2: Merge feature (no fast-forward to preserve history)
git merge --no-ff feature/user-profile-page

# Step 3: Push to develop
git push origin develop

# Step 4: Clean up branches
git branch -d feature/user-profile-page                    # Delete local
git push origin --delete feature/user-profile-page         # Delete remote
```

**Pre-merge validation:**

- All tests passing
- Code reviewed (if team process requires)
- No merge conflicts with develop
- Feature is complete (no partial merges)

---

### Release Branch Complete Workflow

#### 1. Starting a Release

```bash
# Step 1: Create release branch from develop
git checkout develop
git pull origin develop
git checkout -b release/v1.2.0

# Step 2: Update version numbers
# Edit package.json, version files, etc.
# Example for Node.js:
npm version 1.2.0 --no-git-tag-version

# Step 3: Commit version bump
git commit -am "🔖 release: bump version to 1.2.0"

# Step 4: Push release branch
git push -u origin release/v1.2.0
```

#### 2. Release Preparation

On the release branch, only accept:

- Bug fixes (no new features!)
- Documentation updates
- Version number adjustments
- Build configuration tweaks

```bash
# Example release preparation commit
git commit -m "$(cat <<'EOF'
🐛 fix(build): correct production environment variables

Fixed missing API endpoint configuration for production.

Co-Authored-By: AOJDevStudio
EOF
)"
```

#### 3. Finishing a Release

```bash
# Step 1: Merge to main (production)
git checkout main
git pull origin main
git merge --no-ff release/v1.2.0

# Step 2: Tag the release
git tag -a v1.2.0 -m "Release v1.2.0"

# Step 3: Push main with tags
git push origin main --tags

# Step 4: Merge back to develop (to include any release fixes)
git checkout develop
git pull origin develop
git merge --no-ff release/v1.2.0

# Step 5: Push develop
git push origin develop

# Step 6: Clean up release branch
git branch -d release/v1.2.0
git push origin --delete release/v1.2.0
```

**Critical checks before finishing:**

- All release tests passed
- Changelog updated
- Documentation reflects new version
- No pending commits on release branch
- Stakeholder approval obtained

---

### Hotfix Branch Complete Workflow

#### 1. Starting a Hotfix

```bash
# Step 1: Branch from main (NOT develop)
git checkout main
git pull origin main
git checkout -b hotfix/critical-auth-bug

# Step 2: Push to remote
git push -u origin hotfix/critical-auth-bug
```

**Hotfix urgency indicators:**

- 🚨 Site down / Service unavailable
- 🔐 Security vulnerability discovered
- 💥 Data corruption or loss
- ⚠️ Critical feature broken in production

#### 2. Implementing the Fix

```bash
# Make minimal changes to fix the issue
git add <fixed-files>
git commit -m "$(cat <<'EOF'
🐛 fix(auth): prevent null pointer exception on logout

Added null check before accessing user session object.
Fixes production error affecting 15% of users.

Co-Authored-By: AOJDevStudio
EOF
)"
```

**Hotfix principles:**

- Minimal scope (fix only the immediate issue)
- No refactoring or improvements
- Test thoroughly before merging
- Document the issue and fix

#### 3. Finishing a Hotfix

```bash
# Step 1: Bump patch version
git checkout hotfix/critical-auth-bug
# Update version (e.g., 1.2.0 → 1.2.1)
npm version patch --no-git-tag-version
git commit -am "🔖 hotfix: bump version to 1.2.1"

# Step 2: Merge to main
git checkout main
git merge --no-ff hotfix/critical-auth-bug

# Step 3: Tag the hotfix
git tag -a v1.2.1 -m "Hotfix v1.2.1 - Critical auth bug fix"

# Step 4: Push main with tags
git push origin main --tags

# Step 5: Merge to develop
git checkout develop
git merge --no-ff hotfix/critical-auth-bug

# Step 6: Push develop
git push origin develop

# Step 7: Clean up
git branch -d hotfix/critical-auth-bug
git push origin --delete hotfix/critical-auth-bug
```

---

## Commit Message Standards

### Conventional Commits with Emoji Format

Every commit must follow this structure:

```
<emoji> <type>(<scope>): <subject>

[optional body]

[optional footer]

Co-Authored-By: AOJDevStudio
```

### Detailed Breakdown

**Emoji:** Visual indicator of commit type (see emoji-commit-ref.yaml)

**Type:** Category of change

- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation only
- `style` - Code style (formatting, missing semicolons, etc.)
- `refactor` - Code change that neither fixes bug nor adds feature
- `perf` - Performance improvement
- `test` - Adding or updating tests
- `chore` - Tooling, dependencies, config
- `ci` - CI/CD changes
- `build` - Build system changes
- `revert` - Revert previous commit

**Scope:** Component affected (optional but recommended)

- Examples: `auth`, `api`, `ui`, `database`, `config`
- Use parentheses: `feat(auth)`

**Subject:** Brief description

- Imperative mood: "add feature" not "added feature"
- No capitalization of first letter
- No period at the end
- Max 72 characters

**Body:** Detailed explanation (optional)

- Wrap at 72 characters
- Explain what and why, not how
- Separate from subject with blank line

**Footer:** Metadata (optional)

- Breaking changes: `BREAKING CHANGE: description`
- Issue references: `Fixes #123`, `Closes #456`
- Co-authorship (required): `Co-Authored-By: AOJDevStudio`

### Examples of Good Commits

```bash
# Simple feature
✨ feat(profile): add avatar upload functionality

# Bug fix with details
🐛 fix(api): handle null response from external service

Added defensive null checks and fallback to cached data
when external API returns unexpected null values.

Fixes #234

Co-Authored-By: AOJDevStudio

# Breaking change
💥 feat(auth)!: migrate to JWT from session-based auth

BREAKING CHANGE: All existing sessions will be invalidated.
Users must log in again after deployment.

Migration guide: docs/AUTH_MIGRATION.md

Co-Authored-By: AOJDevStudio

# Documentation update
📝 docs(readme): add installation instructions for Windows

# Performance improvement
⚡ perf(database): add index on user_id column

Reduces query time from 2.3s to 45ms for user lookups.

Co-Authored-By: AOJDevStudio
```

### Emoji Reference

**Always reference:** `${PAI_DIR}/ai-docs/templates/emoji-commit-ref.yaml`

Common emojis quick reference:

- ✨ `:sparkles:` - New feature
- 🐛 `:bug:` - Bug fix
- 🔒 `:lock:` - Security fix
- 📝 `:memo:` - Documentation
- 🚀 `:rocket:` - Performance
- ♻️ `:recycle:` - Refactoring
- ✅ `:white_check_mark:` - Tests
- 🔧 `:wrench:` - Configuration
- 💄 `:lipstick:` - UI/styling
- 🔖 `:bookmark:` - Release/version tags

---

## Release Management

### Semantic Versioning

Format: `vMAJOR.MINOR.PATCH` (e.g., `v1.2.3`)

**Version Bump Rules:**

**MAJOR** (v1.0.0 → v2.0.0):

- Breaking API changes
- Incompatible changes to public interfaces
- Removal of deprecated features
- Major architectural changes
- Indicators in commits: `BREAKING CHANGE:` in footer

**MINOR** (v1.0.0 → v1.1.0):

- New features (backwards compatible)
- New functionality added
- Deprecations (but not removals)
- Indicators in commits: `feat:` type

**PATCH** (v1.0.0 → v1.0.1):

- Bug fixes
- Security patches
- Performance improvements
- Documentation updates
- Indicators in commits: `fix:`, `perf:`, `docs:`

### Automatic Version Detection

Analyze commits since last release to suggest version bump:

```bash
# Get commits since last tag
git log $(git describe --tags --abbrev=0)..HEAD --oneline

# Look for:
# - "BREAKING CHANGE:" → MAJOR bump required
# - "feat:" → MINOR bump required
# - "fix:" / "perf:" → PATCH bump sufficient
```

### Changelog Generation

Generate changelog from conventional commits:

```bash
# List all features since last release
git log $(git describe --tags --abbrev=0)..HEAD --grep="^✨ feat" --oneline

# List all fixes
git log $(git describe --tags --abbrev=0)..HEAD --grep="^🐛 fix" --oneline

# Format into CHANGELOG.md:
## [vX.Y.Z] - YYYY-MM-DD

### Added
- New feature 1 (#PR_NUM)
- New feature 2 (#PR_NUM)

### Fixed
- Bug fix 1 (#PR_NUM)
- Bug fix 2 (#PR_NUM)

### Changed
- Refactoring 1 (#PR_NUM)
```

---

## Conflict Resolution

### Detecting Conflicts

```bash
# Conflicts appear during merge
git merge feature/user-profile
# Output: CONFLICT (content): Merge conflict in src/app.js

# Check status
git status
# Shows:
# Unmerged paths:
#   both modified: src/app.js
```

### Understanding Conflict Markers

```javascript
// Example conflict in code
<<<<<<< HEAD (current branch - develop)
function login(username, password) {
  return authenticateUser(username, password);
}
=======
function login(email, password) {
  return authenticateWithEmail(email, password);
}
>>>>>>> feature/email-login (incoming branch)
```

**Markers explained:**

- `<<<<<<< HEAD` - Your current branch's version
- `=======` - Separator
- `>>>>>>> branch-name` - Incoming branch's version

### Resolution Process

**Step 1: Identify all conflicting files**

```bash
git status | grep "both modified"
```

**Step 2: Open each file and resolve conflicts**

Choose one of:

- Keep current version (remove incoming changes)
- Keep incoming version (remove current changes)
- Combine both versions
- Write entirely new code

**Step 3: Remove conflict markers**

Ensure no `<<<<<<<`, `=======`, or `>>>>>>>` remain in file.

**Step 4: Test the resolution**

```bash
# Run tests to verify resolution didn't break functionality
npm test  # or your test command
```

**Step 5: Mark as resolved**

```bash
git add src/app.js  # Add each resolved file
```

**Step 6: Complete the merge**

```bash
git commit  # Opens editor with merge commit message
# Or specify message:
git commit -m "🔀 merge: resolve conflicts in feature/email-login"
```

### Complex Conflict Strategies

**Multiple conflicting files:**

```bash
# Resolve one at a time, testing after each
git add resolved-file-1.js
npm test
git add resolved-file-2.js
npm test
git commit
```

**Accept all from one side:**

```bash
# Keep current branch (ours)
git checkout --ours conflicting-file.js
git add conflicting-file.js

# Keep incoming branch (theirs)
git checkout --theirs conflicting-file.js
git add conflicting-file.js
```

**Abort merge if needed:**

```bash
git merge --abort  # Returns to pre-merge state
```

---

## Error Handling

### Common Git Flow Errors

#### 1. Direct Push to Protected Branch

**Error:**

```
! [remote rejected] main -> main (protected branch hook declined)
```

**Solution:**

```bash
❌ You attempted to push directly to a protected branch.

✅ Correct workflow:
1. Create a feature branch:
   git checkout -b feature/your-change

2. Make your changes and commit

3. Push feature branch:
   git push -u origin feature/your-change

4. Create a pull request to merge into main/develop
```

#### 2. Merge Conflicts

**Error:**

```
CONFLICT (content): Merge conflict in src/app.js
Automatic merge failed; fix conflicts and then commit the result.
```

**Solution:**

```bash
⚠️  Merge conflicts detected in:
   - src/app.js
   - src/utils/auth.js

🔧 Steps to resolve:
1. Open each conflicting file
2. Look for conflict markers (<<<<<<<, =======, >>>>>>>)
3. Edit to resolve conflicts
4. Remove conflict markers
5. Test your changes
6. Stage resolved files: git add <file>
7. Complete merge: git commit
```

#### 3. Invalid Branch Name

**Error:**
User creates branch without proper Git Flow prefix.

**Detection:**

```bash
git branch | grep -v "feature\|release\|hotfix\|main\|develop"
```

**Solution:**

```bash
❌ Invalid branch name: "my-feature"

✅ Use Git Flow naming conventions:
   - feature/descriptive-name  (for new features)
   - release/vX.Y.Z            (for releases)
   - hotfix/descriptive-name   (for urgent fixes)

To rename your branch:
   git branch -m feature/my-feature
   git push -u origin feature/my-feature
```

#### 4. Forgotten Remote Tracking

**Error:**

```
fatal: The current branch feature/login has no upstream branch.
```

**Solution:**

```bash
⚠️  Your branch isn't tracking a remote branch.

✅ Set up tracking:
   git push -u origin feature/login

This enables:
   - git push (without specifying remote)
   - git pull (without specifying remote)
   - Branch status in git status
```

#### 5. Dirty Working Directory

**Error:**
Attempting to switch branches with uncommitted changes.

**Solution:**

```bash
❌ Cannot switch branches with uncommitted changes.

Choose one:
1. Commit your changes:
   git add .
   git commit -m "✨ feat: work in progress"

2. Stash your changes (temporary storage):
   git stash
   # Switch branches, then restore:
   git stash pop

3. Discard your changes (⚠️ DESTRUCTIVE):
   git reset --hard HEAD
```

---

## Pull Request Workflows

### Creating a Pull Request

**Using GitHub CLI (`gh`):**

```bash
# Ensure branch is pushed
git push origin feature/user-profile

# Create PR with full details
gh pr create --title "Add user profile page" --body "$(cat <<'EOF'
## Summary
- Created new user profile page with avatar upload
- Implemented profile editing functionality
- Added profile settings management

## Type of Change
- [x] Feature
- [ ] Bug Fix
- [ ] Hotfix
- [ ] Release

## Test Plan
1. Navigate to /profile
2. Upload new avatar image
3. Edit profile information
4. Verify changes persist after page reload

## Related Issues
Closes #123

## Checklist
- [x] Tests passing locally
- [x] No merge conflicts with develop
- [x] Documentation updated in README.md
- [ ] Code reviewed by team member

## Screenshots
(Add if UI changes)

Co-Authored-By: AOJDevStudio
EOF
)"
```

### PR Body Template Sections

**Summary:**

- Bullet points of key changes
- High-level overview of what PR accomplishes

**Type of Change:**

- Checkbox indicating PR category
- Helps reviewers understand scope

**Test Plan:**

- Step-by-step testing instructions
- Expected outcomes for each step
- Edge cases to verify

**Related Issues:**

- Links to GitHub issues using `Closes #NUM` or `Fixes #NUM`
- Automatically closes issues when PR merges

**Checklist:**

- Pre-merge verification items
- Ensures nothing is forgotten

**Screenshots/Videos:** (if applicable)

- Visual changes require visual proof
- Before/after comparisons helpful

### PR Labels and Reviewers

```bash
# Add labels
gh pr edit --add-label "feature,needs-review"

# Request reviewers
gh pr edit --add-reviewer username1,username2

# Assign to yourself
gh pr edit --add-assignee @me
```

### PR Merge Strategies

**Squash and Merge** (recommended for features):

- Combines all commits into one
- Keeps main/develop history clean
- Use when: Many small WIP commits in feature

**Merge Commit** (recommended for releases):

- Preserves complete history
- Shows all individual commits
- Use when: Commits are already well-organized

**Rebase and Merge** (use with caution):

- Replays commits on top of base branch
- Linear history
- Use when: Very experienced with git

---

## Best Practices

### DO ✅

**Before Creating Branches:**

- Always pull latest from base branch
- Verify you're on correct base branch
- Check for uncommitted changes

**During Development:**

- Commit frequently with meaningful messages
- Use emoji commits consistently
- Write descriptive commit bodies for complex changes
- Test before committing

**Before Merging:**

- Run full test suite
- Resolve all merge conflicts
- Review your own changes (self-review)
- Update documentation if needed

**After Merging:**

- Delete feature/release/hotfix branches
- Verify merge succeeded in target branch
- Monitor for issues in integration

**General:**

- Keep feature branches short-lived (days, not weeks)
- One logical change per commit
- Write commit messages for future developers
- Reference issue numbers in commits

### DON'T ❌

**Never:**

- Commit directly to main or develop
- Force push to shared branches (`git push -f`)
- Rewrite history on public branches
- Merge without running tests
- Create ambiguous branch names
- Leave branches undeleted after merging
- Commit sensitive data (keys, passwords, tokens)
- Merge partial/incomplete features

**Avoid:**

- Large multi-purpose commits
- Vague commit messages ("fix stuff", "updates")
- Merging without reviewing changes
- Working on multiple unrelated features in one branch
- Long-lived feature branches (merge frequently)

### Code Review Guidelines

**As Author:**

- Keep PRs small and focused
- Provide context in PR description
- Respond to feedback promptly
- Don't take criticism personally
- Update PR based on feedback

**As Reviewer:**

- Review promptly (within 24 hours)
- Be constructive and specific
- Ask questions to understand intent
- Approve when satisfied, not perfect
- Focus on logic, not style (use linters for style)

---

## Troubleshooting

### "I committed to the wrong branch"

```bash
# Option 1: Move commit to correct branch
git checkout correct-branch
git cherry-pick <commit-hash>
git checkout wrong-branch
git reset --hard HEAD~1

# Option 2: Create new branch from current commit
git branch feature/new-branch
git reset --hard HEAD~1
git checkout feature/new-branch
```

### "I need to undo my last commit"

```bash
# Keep changes, undo commit
git reset --soft HEAD~1

# Discard changes and commit
git reset --hard HEAD~1
```

### "I pushed but need to undo"

```bash
# Create revert commit (safe for shared branches)
git revert <commit-hash>
git push

# Force push (⚠️ ONLY if you're sole developer)
git reset --hard HEAD~1
git push --force
```

### "My branch is behind develop"

```bash
# Update your feature branch with latest develop
git checkout feature/your-branch
git fetch origin
git merge origin/develop

# Or use rebase (cleaner history, but more complex)
git rebase origin/develop
```

---

## Workflow Decision Tree

```
START: Need to make changes?
│
├─ Is it a production emergency? → YES → Create hotfix branch from main
│                                  │
│                                  NO
│                                  ↓
├─ Is it a new feature? → YES → Create feature branch from develop
│                        │
│                        NO
│                        ↓
├─ Is it a release? → YES → Create release branch from develop
│                    │
│                    NO → You might be on the wrong workflow!
│
└─ Complete work → Run tests → Merge per Git Flow rules → Delete branch
```

---

## Additional Resources

**External Documentation:**

- [Git Flow Original](https://nvie.com/posts/a-successful-git-branching-model/)
- [Conventional Commits Spec](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)

**Internal References:**

- Emoji commit reference: `${PAI_DIR}/ai-docs/templates/emoji-commit-ref.yaml`
- Quick command cheatsheet: `SKILL.md`

---

**Last Updated:** 2025-01-05