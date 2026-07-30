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

## Requirements

Python 3.9 or newer. Note that this is a deliberate change from the `>=3.8` floor
of earlier standalone copies of this script: Python 3.8 reached end of life.

## Development

Run the test suite:

```bash
uv run --directory changelog pytest
```
