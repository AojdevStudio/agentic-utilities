# Pull Request Workflow

Create and manage pull requests with GitHub CLI, repo-aware target branch detection, and PR-template / issue-link compliance.

## Variables

```bash
PR_TITLE: $ARGUMENTS or auto-generated
CURRENT_BRANCH: $(git branch --show-current)
DEFAULT_BRANCH: detected from GitHub / origin HEAD
INTEGRATION_BRANCH: develop when present, otherwise DEFAULT_BRANCH
TARGET_BRANCH: derived from branch type + repo shape
ISSUE_LINK: actual closing keyword line (e.g. Closes #123) or explicit no-issue-required marker
```

## Workflow

### 1. Pre-PR Checks

```bash
CURRENT_BRANCH=$(git branch --show-current)
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q '.defaultBranchRef.name' 2>/dev/null || git remote show origin | sed -n '/HEAD branch/s/.*: //p')
if git show-ref --verify --quiet refs/heads/develop || git ls-remote --exit-code --heads origin develop >/dev/null 2>&1; then
  INTEGRATION_BRANCH=develop
else
  INTEGRATION_BRANCH="$DEFAULT_BRANCH"
fi

case "$CURRENT_BRANCH" in
  feature/*) TARGET_BRANCH="$INTEGRATION_BRANCH" ;;
  release/*|hotfix/*) TARGET_BRANCH="$DEFAULT_BRANCH" ;;
  *) TARGET_BRANCH="$DEFAULT_BRANCH" ;;
esac

# Ensure branch is pushed
git push -u origin "$CURRENT_BRANCH"

# Check for unpushed commits
git log "origin/$CURRENT_BRANCH"..HEAD --oneline

# Check for merge conflicts with target
git fetch origin
git merge-base --is-ancestor "origin/$TARGET_BRANCH" HEAD || echo "May have conflicts"
```

Stop if:
- the working tree is dirty in a way that would make the PR misleading,
- the branch obviously targets the wrong base branch,
- or the repo requires issue linkage and no valid issue path exists yet.

---

### 2. Detect Repo PR Requirements

Inspect the repo before generating the PR body:

```bash
TEMPLATE_FILE=""
for f in .github/PULL_REQUEST_TEMPLATE.md .github/pull_request_template.md; do
  [ -f "$f" ] && TEMPLATE_FILE="$f" && break
done

rg -n "Closes #|Fixes #|Resolves #|No issue required|closing keyword|auto-close|issue link" \
  .github CONTRIBUTING.md docs/CONTRIBUTING.md 2>/dev/null || true
```

Rules:

- If `TEMPLATE_FILE` exists, mirror its sections and wording.
- If the repo has a metadata check like strict `PR issue link`, the PR body must contain either:
  - a real closing keyword: `Closes #123`, `Fixes #123`, or `Resolves #123`, or
  - an explicit no-issue marker exactly matching the repo template, such as `- [x] No issue required ...`.
- Never leave placeholders like `Closes #`, `#ISSUE_NUM`, or `Closes #123 (if applicable)` without replacing them.

---

### 3. Determine the Issue Link Strategy

Every PR must resolve "what GH issue does this address?" before creation. Work through this order and stop at the first hit — don't skip to "ask the user" without trying detection first.

**Preferred order:**

1. **User-supplied** — issue number the user gave you in the request.
2. **Auto-detect from branch name** — extract trailing/leading issue number:
   ```bash
   ISSUE_NUM=$(echo "$CURRENT_BRANCH" | grep -oE '(^|[/_#-])([0-9]{1,6})([/_-]|$)' | grep -oE '[0-9]+' | head -1)
   ```
3. **Auto-detect from commit messages** — scan for closing keywords already authored:
   ```bash
   git log "origin/$TARGET_BRANCH"..HEAD --pretty=%B | grep -oE '(Fixes|Closes|Resolves|Refs) #[0-9]+' | head -1
   ```
4. **Open issues assigned to user** — fall back to listing for human pick:
   ```bash
   gh issue list --assignee @me --state open --json number,title --limit 20
   ```
5. **No-issue path** — if the repo template allows `No issue required` and this is docs-only / dependency-only / housekeeping, mark that checkbox explicitly. Do not invent this path if the template doesn't offer it.
6. **Stop and ask** — if the repo requires an issue and none of the above resolves it, stop and tell the user to open or specify the issue first.

**Validate the issue exists** (steps 1–4) before using it:

```bash
gh issue view "$ISSUE_NUM" --json number,title,state >/dev/null 2>&1 \
  || { echo "Issue #$ISSUE_NUM not found in this repo — re-detect or ask user"; exit 1; }
```

Sanity check after choosing:

```bash
printf '%s
' "$ISSUE_LINK"
# Must be one of:
#   Closes #123
#   Fixes #123
#   Resolves #123
#   Refs #123                                  (related, does not close)
#   - [x] No issue required ...                (only when template allows)
```

---

### 4. Generate PR Content

Analyze commits to generate the summary:

```bash
# Get commits in this branch
git log "origin/$TARGET_BRANCH"..HEAD --oneline

# Get changed files
git diff "origin/$TARGET_BRANCH" --name-only
```

Build the PR body so it satisfies both the repo template and any issue-link checks.

**Template for repos with strict PR-issue-link metadata checks:**

```md
## Summary
- Key change 1
- Key change 2
- Key change 3

## Linked issues
Closes #123

- [ ] No issue required (docs-only, dependency-only, or housekeeping)

## Verification
- bun run typecheck
- cd app && bun run test
```

**No-issue-required variant:**

```md
## Summary
- Documentation cleanup
- No product behavior changed

## Linked issues
- [x] No issue required (docs-only, dependency-only, or housekeeping)

## Verification
- bun run lint
```

---

### 5. Create Pull Request

Prefer `--body-file` over giant inline heredocs when repo templates matter.

```bash
cat >/tmp/pr-body.md <<'EOF'
## Summary
- Key change 1
- Key change 2

## Linked issues
Closes #123

- [ ] No issue required (docs-only, dependency-only, or housekeeping)

## Verification
- test command 1
- test command 2
EOF

gh pr create \
  --base "$TARGET_BRANCH" \
  --title "$PR_TITLE" \
  --body-file /tmp/pr-body.md
```

---

### 6. Verify PR Metadata Immediately

Do not assume GitHub parsed the body the way you intended.

```bash
gh pr view --json number,url,body,baseRefName,closingIssuesReferences
```

Checks:

- If `baseRefName` is wrong, fix the PR before doing anything else.
- If the repo expects an issue-closing keyword and `closingIssuesReferences` is empty, the body is malformed or missing the real issue line — fix it immediately.
- If using the no-issue-required path, make sure the checkbox is `[x]`, not `[ ]`.

Fix in place when needed:

```bash
gh pr edit <PR_NUMBER> --body-file /tmp/pr-body.md
```

---

### 7. Add Labels and Reviewers (Optional)

```bash
# Add labels
gh pr edit --add-label "feature,needs-review"

# Request reviewers
gh pr edit --add-reviewer username1,username2

# Assign to yourself
gh pr edit --add-assignee @me
```

---

### 8. Continue to CI Monitoring (Default: Yes)

After creating the PR, **automatically proceed to the CI Monitor & Merge workflow** unless the user explicitly opts out. This is the natural continuation.

```
→ PR created. Monitoring CI and automated reviews...
→ Read: workflows/CIMerge.md and execute Phase 2A
```

If the user says "just create the PR" or "don't merge yet", stop here and report without continuing.

---

## PR Body Guidance

**Summary:**
- Bullet points of key changes
- High-level overview of what the PR accomplishes

**Linked issues:**
- Use `Closes #NUM`, `Fixes #NUM`, or `Resolves #NUM` when tied to an issue
- Use the repo's exact `No issue required` checkbox text when that path is allowed
- Avoid placeholders

**Verification:**
- Concrete local commands actually run
- Keep it truthful and copy-pastable

**Screenshots/Videos:**
- Add when UI changes need visual proof

---

## Merge Strategies

**Squash and Merge** (recommended for features):
- Combines all commits into one
- Keeps target branch history clean
- Use when: Many small WIP commits in feature branches

**Merge Commit** (recommended for releases):
- Preserves complete history
- Shows all individual commits
- Use when: Release commits are already well-organized

**Rebase and Merge** (use with caution):
- Replays commits on top of base branch
- Linear history
- Use when: Very experienced with git and repo policy allows it

---

## Report (if stopping after PR creation)

```md
## Pull Request Created

**Title:** PR_TITLE
**Branch:** CURRENT_BRANCH → TARGET_BRANCH
**URL:** [PR link from gh output]

**Issue Link:** ISSUE_LINK

**Summary:**
- X commits
- Y files changed
- Z insertions, W deletions

**Next Steps:**
- CI monitoring available: /GitWorkflow merge
- Or monitor manually at the PR URL
```
