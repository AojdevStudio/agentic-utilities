# Use harness-isolated skill inventories by default

We use harness-isolated skill inventories by default: canonical shared skills live in `skills/<name>/`, while daily-use copies are installed into the specific harness that should run them. This replaces the old global-first symlink workflow because Pi, Codex, and Claude Code often need different tool names, paths, prompts, and model assumptions; symlinks remain available only when coupled behavior is intentional and documented.
