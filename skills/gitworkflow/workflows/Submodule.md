# Submodule Workflow

Manage git submodules: add, update, remove, and sync repositories as submodules.

## Usage

```
/GitWorkflow submodule add <url> [path]     # Add a repo as submodule
/GitWorkflow submodule update               # Update all submodules to latest
/GitWorkflow submodule remove <path>        # Remove a submodule
/GitWorkflow submodule status               # Show submodule status
/GitWorkflow submodule sync                 # Sync submodule URLs from .gitmodules
```

## Variables

```
SUBMODULE_ACTION: {{first word of ARGUMENTS}}
SUBMODULE_URL: {{extract URL from ARGUMENTS}}
SUBMODULE_PATH: {{extract path from ARGUMENTS, or derive from URL}}
```

---

## Action: add

Add an external repository as a submodule.

### Workflow

1. **Validate inputs:**
   - Check URL is provided
   - If path not provided, derive from URL: `repo-name` from `github.com/org/repo-name.git`

2. **Check for conflicts:**
   ```bash
   # Ensure path doesn't already exist
   test -e "$SUBMODULE_PATH" && echo "ERROR: Path already exists" && exit 1

   # Ensure not already a submodule
   git config --file .gitmodules --get "submodule.$SUBMODULE_PATH.url" && echo "ERROR: Already a submodule"
   ```

3. **Add submodule:**
   ```bash
   git submodule add $SUBMODULE_URL $SUBMODULE_PATH
   ```

4. **Initialize and fetch:**
   ```bash
   git submodule update --init --recursive $SUBMODULE_PATH
   ```

5. **Update .gitignore if needed:**
   - Check if path was in .gitignore
   - If so, remove it (submodules should be tracked)
   - Use `AskUserQuestion` to confirm removal

6. **Display result:**
   ```
   ✅ Submodule added: $SUBMODULE_PATH

   Remote: $SUBMODULE_URL
   Commit: $(cd $SUBMODULE_PATH && git rev-parse --short HEAD)

   Files staged:
   - .gitmodules
   - $SUBMODULE_PATH

   💡 Run `/git:commit` to commit the submodule addition
   ```

### Path Conventions

| Repo Type | Suggested Path |
|-----------|---------------|
| Related project repos | `repos/<name>` |
| Shared libraries | `packages/<name>` or `libs/<name>` |
| Documentation repos | `docs/<name>` |
| Tools/scripts | `tools/<name>` |

---

## Action: update

Update all submodules to their latest remote commits.

### Workflow

1. **Fetch latest from all remotes:**
   ```bash
   git submodule foreach --recursive 'git fetch origin'
   ```

2. **Check for updates:**
   ```bash
   git submodule foreach --recursive 'git log HEAD..origin/$(git rev-parse --abbrev-ref HEAD) --oneline'
   ```

3. **If updates available, ask user:**
   - Use `AskUserQuestion`: "Update submodules to latest?"
   - Options: "Yes, update all", "Let me choose which ones", "Cancel"

4. **Update submodules:**
   ```bash
   git submodule update --remote --merge
   ```

5. **Display results:**
   ```bash
   git submodule status
   ```

6. **Stage and prompt for commit:**
   ```
   💡 Submodule pointers updated. Run `/git:commit` to commit the updates.
   ```

---

## Action: remove

Remove a submodule from the repository.

### Workflow

1. **Validate submodule exists:**
   ```bash
   git config --file .gitmodules --get "submodule.$SUBMODULE_PATH.url"
   ```

2. **Confirm with user:**
   - Use `AskUserQuestion`: "Remove submodule at $SUBMODULE_PATH? This will delete the directory."
   - Show current commit being tracked

3. **Remove submodule:**
   ```bash
   # De-init the submodule
   git submodule deinit -f $SUBMODULE_PATH

   # Remove from .git/modules
   rm -rf .git/modules/$SUBMODULE_PATH

   # Remove from working tree and index
   git rm -f $SUBMODULE_PATH
   ```

4. **Display result:**
   ```
   ✅ Submodule removed: $SUBMODULE_PATH

   💡 Run `/git:commit` to commit the removal
   ```

---

## Action: status

Show detailed status of all submodules.

### Workflow

1. **Check for submodules:**
   ```bash
   test -f .gitmodules || echo "No submodules in this repository"
   ```

2. **Display status table:**
   ```bash
   git submodule status --recursive
   ```

3. **Interpret and display:**

   | Prefix | Meaning |
   |--------|---------|
   | ` ` (space) | Clean, at recorded commit |
   | `+` | Submodule has new commits (need to commit parent) |
   | `-` | Not initialized (run `git submodule update --init`) |
   | `U` | Merge conflict |

4. **Check for dirty content:**
   ```bash
   git submodule foreach 'git status --porcelain'
   ```

5. **Display formatted output:**
   ```
   ## Submodules Status

   | Submodule | Status | Commit | Dirty |
   |-----------|--------|--------|-------|
   | acp-church-media | clean | abc1234 | No |
   | repos/daemon-mcp | ahead | def5678 | Yes (3 files) |
   | repos/playlist-transcripts | clean | ghi9012 | No |

   💡 Dirty submodules need: `cd <path> && git commit` or `/git:commit` (handles automatically)
   💡 Ahead submodules need: parent commit to update pointer
   ```

---

## Action: sync

Sync submodule remote URLs after editing .gitmodules.

### Workflow

1. **Sync URLs:**
   ```bash
   git submodule sync --recursive
   ```

2. **Display synced URLs:**
   ```bash
   git submodule foreach 'echo "$name: $(git remote get-url origin)"'
   ```

---

## Error Handling

| Error | Action |
|-------|--------|
| URL invalid | Validate URL format, suggest HTTPS or SSH |
| Path exists (not submodule) | Ask to convert or choose different path |
| Network error | Retry with SSH if HTTPS fails |
| Submodule not found | Show available submodules from .gitmodules |
| Permission denied | Check SSH keys, suggest HTTPS fallback |

---

## Integration with Commit Workflow

The Commit workflow (Phase 0) automatically handles dirty submodules:

1. Detects modified content in submodules
2. Commits changes inside submodule first
3. Pushes submodule to its remote
4. Then updates parent repo's submodule pointer

This Submodule workflow handles **structural changes** (add/remove/sync), while Commit handles **content changes**.
