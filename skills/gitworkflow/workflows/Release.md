# Release Workflow

Manage releases with semantic versioning and changelog generation.

## Variables

```
VERSION: $ARGUMENTS (e.g., 1.2.0)
RELEASE_TYPE: {{major|minor|patch}} - auto-detected from commits
```

## Workflow

### 1. Determine Version Bump

Analyze commits since last release to suggest version bump:

```bash
# Get commits since last tag
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

**Version Bump Rules:**

| Commit Type | Version Bump | Example |
|-------------|--------------|---------|
| `BREAKING CHANGE:` in footer | MAJOR | v1.0.0 → v2.0.0 |
| `feat:` commits | MINOR | v1.0.0 → v1.1.0 |
| `fix:`, `perf:`, `docs:` | PATCH | v1.0.0 → v1.0.1 |

```bash
# Check for breaking changes
git log $(git describe --tags --abbrev=0)..HEAD | grep -i "BREAKING CHANGE"

# Check for features
git log $(git describe --tags --abbrev=0)..HEAD --grep="^✨ feat"

# Check for fixes
git log $(git describe --tags --abbrev=0)..HEAD --grep="^🐛 fix"
```

---

### 2. Create Release Branch

```bash
# Create release branch from develop
git checkout develop
git pull origin develop
git checkout -b release/vVERSION
```

---

### 3. Update Version Files

Update version in project files:

**Node.js:**
```bash
npm version VERSION --no-git-tag-version
# or
pnpm version VERSION --no-git-tag-version
```

**Python:**
```bash
# Update __version__ in __init__.py or pyproject.toml
```

**Rust:**
```bash
# Update version in Cargo.toml
```

Commit version bump:
```bash
git commit -am "🔖 release: bump version to VERSION"
```

---

### 4. Generate Changelog (MANDATORY — Uses `changelog` CLI)

**⚠️ This step is GATED. You MUST use the `changelog` CLI tool. Do NOT manually construct changelogs.**

Generate changelog from conventional commits using the automated tool:

```bash
# Preview what will be generated (dry-run first)
changelog VERSION --auto --dry-run

# Generate and update CHANGELOG.md (non-interactive for automation)
changelog VERSION --auto --force
```

The `changelog` CLI tool (`~/.local/bin/changelog`) automatically:
- Analyzes all commits since the last git tag
- Groups by type: Added, Fixed, Changed, Deprecated, Removed, Security
- Detects breaking changes for MAJOR version bumps
- Extracts PR numbers from commit messages
- Creates backup of existing CHANGELOG.md
- Updates version comparison links at the bottom
- Follows Keep a Changelog format

**If changelog tool is not available**, fall back to manual generation:

```bash
# List all features since last release
git log $(git describe --tags --abbrev=0)..HEAD --grep="^✨ feat" --oneline

# List all fixes
git log $(git describe --tags --abbrev=0)..HEAD --grep="^🐛 fix" --oneline

# List breaking changes
git log $(git describe --tags --abbrev=0)..HEAD --grep="BREAKING CHANGE" --oneline
```

Commit changelog:
```bash
git add CHANGELOG.md
git commit -m "📝 docs: update changelog for vVERSION"
```

---

### 5. Push Release Branch

```bash
git push -u origin release/vVERSION
```

---

### 6. Finalize Release

After testing and approval:

```bash
# Merge to main
git checkout main
git pull origin main
git merge --no-ff release/vVERSION

# Tag the release
git tag -a vVERSION -m "Release vVERSION"

# Push main with tags
git push origin main --tags

# Merge back to develop
git checkout develop
git pull origin develop
git merge --no-ff release/vVERSION
git push origin develop

# Clean up
git branch -d release/vVERSION
git push origin --delete release/vVERSION
```

---

## Semantic Versioning Guide

Format: `vMAJOR.MINOR.PATCH` (e.g., v1.2.3)

**MAJOR** (v1.0.0 → v2.0.0):
- Breaking API changes
- Incompatible changes to public interfaces
- Removal of deprecated features
- Major architectural changes

**MINOR** (v1.0.0 → v1.1.0):
- New features (backwards compatible)
- New functionality added
- Deprecations (but not removals)

**PATCH** (v1.0.0 → v1.0.1):
- Bug fixes
- Security patches
- Performance improvements
- Documentation updates

---

## Report

```
## Release Created

**Version:** vVERSION
**Type:** RELEASE_TYPE bump
**Tag:** vVERSION
**Branch:** release/vVERSION

**Changelog:**
[summary of changes]

**Next Steps:**
1. Test release branch
2. Get approval
3. Run: Finish release workflow
```
