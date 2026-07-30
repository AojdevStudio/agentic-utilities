# Changelog Updater

Install the tool with uv:

```bash
uv tool install "git+https://github.com/AojdevStudio/agentic-utilities#subdirectory=changelog"
```

Usage:

```bash
# Automatically analyze commits for an explicit version
changelog 1.1.0 --auto

# Enter changes interactively
changelog 1.1.0 --manual

# Preview an automatic update without modifying files
changelog 1.1.0 --auto --dry-run

# Regenerate the Unreleased section
changelog --unreleased
```
