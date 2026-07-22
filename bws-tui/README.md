# bws-tui

Interactive TUI and script-friendly CLI wrapper around the [Bitwarden Secrets Manager](https://bitwarden.com/products/secrets-manager/) `bws` CLI.

## Prerequisites — read first

`bws-tui` is a **wrapper**, not a standalone client. It shells out to the official `bws` binary, so you need:

1. The [`bws` CLI](https://github.com/bitwarden/sdk-sm/releases) installed and on `PATH` (developed against v2.1.0).
2. A **machine account access token** exported as `BWS_ACCESS_TOKEN` (create one in the Bitwarden web vault under Secrets Manager → Machine accounts). No login flow exists in this tool — the token does everything.

```bash
export BWS_ACCESS_TOKEN="..."
```

## Install

```bash
cargo install bws-tui
```

## Usage

Bare `bws-tui` launches the TUI:

- **Add** — pick a project (fetched live), enter a key, enter a masked value, done.
- **Search** — fuzzy-filter secrets across all projects as you type; project shown per row. Values are never rendered in lists, and the list reloads every time you open Search so it's always fresh.
  - `Enter` opens the action menu for the selected secret: **Copy value to clipboard** (auto-clears after 30s), **Reveal value**, **Edit secret**, **Delete secret** (two-step), or Cancel.
  - Menu shortcuts: `c` copy · `r` reveal · `e` edit · `d` delete. `Esc` backs out of everything.

Every operation also works non-interactively for scripts and CI:

```bash
bws-tui list                          # id, key, project — never values
bws-tui get --key DB_PASSWORD         # prints the value
bws-tui add --project "API Keys" --key NEW_SECRET --value s3cr3t
cat key.pem | bws-tui add --project "API Keys" --key TLS_PEM   # value from stdin (multiline-safe)
bws-tui edit --key DB_PASSWORD --value newval --note "rotated 2026-07"
bws-tui delete --key OLD_SECRET --yes # --yes required; deletion is permanent
```

`--project` accepts a project name or UUID. Without it, `get`/`edit`/`delete` search all projects and require an unambiguous key match.

## Security notes

- Values are passed to `bws` as **argv** (its only accepted input in v2.1.0) and are therefore briefly visible in `ps`. Single-user machines: acceptable. Multi-user hosts: be aware. <!-- ponytail: bws has no stdin/file input for values; nothing a wrapper can do until upstream adds one -->
- TUI value entry is masked; values never appear in lists or logs.
- Clipboard copies auto-clear after 30s, only if the clipboard still holds the copied value, and only while the app stays open (the clear timer exits with the process).
- Secret buffers use `secrecy`/`zeroize`.

## License

MIT
