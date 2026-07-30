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

## How releases treat the Unreleased section

Cutting a release **replaces** the Unreleased section with the entry generated
from your commit range. It does not promote the existing Unreleased notes into
the release.

The practical consequence: hand-written notes in Unreleased that are not
regenerated from commits are discarded when you release. Anything about to be
lost is printed for review first, in `--dry-run` and under `--force` alike, and
the interactive confirmation says "Replace" rather than "Add" when there is
curated content at stake.

If you keep a hand-curated Unreleased section, copy what you want to keep into
the release entry before running the release, or drive the section from commits
with `changelog --unreleased`.

## Requirements

Python 3.9 or newer. Note that this is a deliberate change from the `>=3.8` floor
of earlier standalone copies of this script: Python 3.8 reached end of life.

## Development

Run the test suite:

```bash
uv run --directory changelog pytest
```
