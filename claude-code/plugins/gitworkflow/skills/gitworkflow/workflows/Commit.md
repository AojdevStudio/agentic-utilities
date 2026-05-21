# Commit Workflow

Smart commit workflow with submodule awareness, hook-aware strategy detection, conventional commit messages, and **auto-push to remote**.

**Default behavior:** Commits are automatically pushed to the remote repository. Use `--no-push` flag to skip pushing.

## Variables

```
COMMIT_OPTIONS: $ARGUMENTS
STRATEGY_MODE: auto-detected
NO_VERIFY: {{if contains COMMIT_OPTIONS "--no-verify"}}true{{else}}false{{endif}}
NO_SUBMODULES: {{if contains COMMIT_OPTIONS "--no-submodules"}}true{{else}}false{{endif}}
NO_PUSH: {{if contains COMMIT_OPTIONS "--no-push"}}true{{else}}false{{endif}}
CUSTOM_MESSAGE: {{extract message from COMMIT_OPTIONS}}
```

## Workflow

### Phase 0: Submodule Detection & Processing

**Skip if `NO_SUBMODULES` is true.**

1. Check if `.gitmodules` file exists in the repository root
2. If submodules exist, run `git submodule status` to detect dirty submodules
   - Look for `+` prefix (submodule has new commits) or `M` in status
   - Also check `git status --porcelain` for `modified: <submodule> (modified content)`
3. **For each dirty submodule:**
   a. Display: "📦 Found dirty submodule: `<submodule-name>` - processing first..."
   b. `cd` into the submodule directory
   c. Run `git status --porcelain` inside the submodule
   d. If uncommitted changes exist:
      - Auto-stage with `git add .`
      - Analyze changes for appropriate commit message
      - Commit with conventional message + emoji
      - Auto-push submodule to remote: `git push`
   e. Return to parent directory
4. After all submodules processed, continue with parent repo workflow

#### Submodule Status Indicators

| Indicator | Meaning | Action |
|-----------|---------|--------|
| `+abc123` | Submodule has new commits not in parent | Commit parent to update pointer |
| ` abc123` | Submodule is clean | No action needed |
| `-abc123` | Submodule not initialized | Run `git submodule update --init` |
| `(modified content)` | Uncommitted changes in submodule | **Commit submodule first** |
| `(untracked content)` | Untracked files in submodule | Stage and commit submodule first |

---

### Phase 1: Parent Repository Analysis

5. Run `git status --porcelain` to analyze current repository state
6. Execute formatting hook analysis to determine optimal commit strategy:

#### Hook-Aware Strategy Detection

Analyze pre-commit hooks to determine commit strategy:

```bash
# Check for formatting hooks
cat .git/hooks/pre-commit 2>/dev/null | grep -E "(prettier|eslint|black|rustfmt)" || echo "no-formatting-hooks"

# Check for husky/lint-staged
cat package.json 2>/dev/null | grep -E "(husky|lint-staged)" || echo "no-husky"
```

**Strategy Selection:**

| Hook Configuration | Strategy | Behavior |
|-------------------|----------|----------|
| No formatting hooks | PARALLEL | Can stage multiple commits independently |
| Formatting hooks (non-aggressive) | COORDINATED | Stage and commit sequentially |
| Aggressive formatting (prettier --write) | HYBRID | Stage all, let hook format, single commit |

7. Check for `--no-verify` flag in `COMMIT_OPTIONS`, skip pre-commit checks if present

---

### Phase 2: Pre-commit Validation

**Skip if `NO_VERIFY` is true.**

8. Run pre-commit validation (if applicable to project type):
   - Node.js: `pnpm lint` (or npm/yarn)
   - Python: `ruff check .` or `black --check .`
   - Rust: `cargo clippy`

9. Validate `.gitignore` configuration:
   - Check for common sensitive files (.env, credentials, etc.)
   - Alert if sensitive files are staged

10. Check for large files (>1MB):
    ```bash
    git diff --cached --name-only | xargs -I{} du -h {} 2>/dev/null | awk '$1 ~ /M|G/ {print}'
    ```

---

### Phase 3: Staging & Commit

11. Auto-stage files with `git add .` if no files currently staged
12. Execute `git diff --staged --name-status` to analyze staged changes
13. Analyze changes for atomic commit splitting opportunities:
    - Group by feature/component
    - Separate docs from code
    - Separate tests from implementation

14. Generate conventional commit message:

#### Commit Message Format

```
<emoji> <type>(<scope>): <description>

[optional body - what and why]

Co-Authored-By: <Your Name or Team>
```

**Type Selection:**
- Analyze changed files to determine type
- Use emoji reference from `<your emoji commit reference>`

| Changed Files | Type | Emoji |
|--------------|------|-------|
| New feature files | feat | ✨ |
| Bug fixes | fix | 🐛 |
| Documentation only | docs | 📝 |
| Test files only | test | ✅ |
| Config/tooling | chore | 🔧 |
| Refactoring | refactor | ♻️ |
| Performance | perf | ⚡ |
| Style/formatting | style | 💄 |

15. Execute commit:
    ```bash
    git commit {{if NO_VERIFY}}--no-verify{{endif}} -m "$(cat <<'EOF'
    <emoji> <type>(<scope>): <description>

    <body if complex changes>

    Co-Authored-By: <Your Name or Team>
    EOF
    )"
    ```

16. If `CUSTOM_MESSAGE` provided, use it instead of auto-generated:
    ```bash
    git commit {{if NO_VERIFY}}--no-verify{{endif}} -m "CUSTOM_MESSAGE"
    ```

17. Display commit summary:
    ```bash
    git log --oneline -1
    git diff --stat HEAD~1
    ```

---

### Phase 4: Auto-Push to Remote

**Skip if `NO_PUSH` is true.**

18. Check if remote tracking branch exists:
    ```bash
    git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null
    ```

19. If tracking branch exists, auto-push:
    ```bash
    git push
    ```

20. If no tracking branch, set upstream and push:
    ```bash
    git push -u origin $(git rev-parse --abbrev-ref HEAD)
    ```

21. Display push confirmation:
    ```bash
    echo "✅ Pushed to remote: $(git rev-parse --abbrev-ref --symbolic-full-name @{u})"
    ```

**Default behavior:** Push is ALWAYS performed automatically unless `--no-push` flag is provided. The workflow completes the full commit→push cycle without user confirmation.

---

### Phase 5: Changelog Summary (GATED)

**⚠️ This phase ALWAYS runs. There is no flag to skip it.** Changelog awareness is non-negotiable.

22. Run changelog dry-run to show accumulated unreleased changes:
    ```bash
    changelog --auto --dry-run 2>&1
    ```

23. If the command succeeds, display the output:
    ```
    📋 Unreleased Changes (since last tag)
    ─────────────────────────────────────
    [dry-run output]

    💡 To update CHANGELOG.md: changelog <version> --auto
    💡 To create a release: use /GitWorkflow release
    ```

24. If no tags exist in the repo:
    ```
    📋 No tags found — changelog will cover all commits.
    💡 Create your first release: changelog 0.1.0 --auto
    ```

25. If `changelog` command is not found:
    ```
    ⚠️ Changelog CLI not found 
    ```
    Continue (non-blocking).

**Why this is gated:** Without changelog awareness, you lose track of what's accumulating between releases. This summary costs ~1 second and provides critical repo context.

---

## Flags

| Flag | Description |
|------|-------------|
| `--no-verify` | Skip pre-commit hooks and validation |
| `--no-submodules` | Skip submodule processing |
| `--no-push` | Skip auto-push (commit only, do not push to remote) |
| `--message "..."` or `-m "..."` | Use custom commit message |

---

## Report

```
## Commit Complete

**Strategy:** STRATEGY_MODE (auto-detected)
**Submodules:** X submodules processed
**Files:** Y files committed
**Pushed:** ✅ origin/BRANCH_NAME (or ⏸️ Skipped with --no-push)
**Changelog:** 📋 Unreleased changes shown

**Commit:**
<git log --oneline -1>

**Stats:**
<git diff --stat HEAD~1>

**Remote:**
<git rev-parse --abbrev-ref --symbolic-full-name @{u}>
```

---

## Error Handling

| Error | Action |
|-------|--------|
| No staged changes | Auto-stage modified files, or warn if working tree clean |
| Pre-commit hook fails | Show error, abort commit (unless --no-verify) |
| Submodule push fails | Warn user, continue with parent commit |
| Large file detected | Warn user, suggest adding to .gitignore |
| Sensitive file staged | Block commit, show warning |
| Changelog tool not found | Warn user, continue (non-blocking) |
| Changelog dry-run fails | Warn user, continue (non-blocking) |
