# Rust Stack

Covers: Rust 1.70+, Cargo workspaces, embedded Rust, web (axum/actix), Tauri.

## Tooling matrix

| Concern | Recommended | Alternatives |
|---------|-------------|--------------|
| Lint | clippy (built-in) | — |
| Format | rustfmt (built-in) | — |
| Test runner | `cargo test` (built-in) | nextest (faster, parallel) |
| Pre-commit | `cargo husky` or native git hooks | pre-commit framework |
| CI | GitHub Actions | sccache for speed |

## Config paths to check

- `Cargo.toml` (workspace root) → `[workspace]`, `[lints]`, `[profile.*]`
- `rustfmt.toml`, `.rustfmt.toml`
- `clippy.toml`, `.clippy.toml`
- `.cargo/config.toml` for build flags

## Pre-commit pattern

`.git/hooks/pre-commit`:
```sh
#!/usr/bin/env bash
set -euo pipefail

# Format check (fails if not formatted)
cargo fmt --all -- --check

# Lint staged
cargo clippy --workspace --all-targets -- -D warnings

# Tests scoped to changed crates would be ideal but cargo doesn't do incremental test selection well.
# For small projects: cargo test --workspace
# For large: skip tests in pre-commit, rely on pre-push or CI
```

For larger workspaces (>30s `cargo build`), use `cargo nextest run` and only test the workspace member containing staged files.

## Test wrapper

`cargo test` is the one-liner. Variants:

```sh
cargo test --workspace --all-features  # everything
cargo test -p my-crate                  # one crate
cargo nextest run --workspace           # 2-3× faster, parallel
```

If the project has integration tests in `tests/` directories that take long (DB, network), gate them behind a feature:
```toml
[features]
integration = []
```

And expose:
```sh
cargo test                       # unit only
cargo test --features integration  # everything
```

## CI pattern (GitHub Actions)

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
```

`Swatinem/rust-cache` is essential — without it, every PR rebuilds from scratch.

## Lints worth encoding in `Cargo.toml`

```toml
[workspace.lints.rust]
unsafe_code = "deny"
unused_imports = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
```

The `[workspace.lints]` section (Rust 1.74+) propagates to all workspace members — single source of truth.

## Rust-specific gotchas to flag

- **Long compile times burn agent attention** — every iteration that waits 30s+ for `cargo build` is harness debt. Recommend `sccache`, `mold` linker, `cargo nextest`.
- **`unwrap()` proliferation** — clippy can catch this, but only if the lint is enabled. Flag projects with lots of `.unwrap()` in non-test code.
- **`unsafe` blocks without `// SAFETY:` comments** — house rule worth encoding via clippy `undocumented_unsafe_blocks`.
- **Macro-heavy code** — agents struggle with derive-macros and proc-macros if the expanded output isn't documented. Recommend `cargo expand` skill for inspection.
- **Workspaces with diverging dependency versions** — `cargo tree -d` finds duplicates. Worth flagging.
- **Tokio runtime mismatches** — multi-runtime panics. Document which runtime the project uses.

## Repo skills worth seeding

- `run-bench` — wraps `cargo bench` with the right baseline comparison
- `expand-macro` — wraps `cargo expand` for the active file
- `update-deps` — wraps `cargo update` + `cargo outdated`

## Bonus signals (for the audit prompt)

- Workspace members count
- Edition (`edition = "2021"`, `"2024"`)
- MSRV (`rust-version`)
- Embedded? (no_std, target triple)
- Tauri / Bevy / Embassy / Axum / Actix detection
- Custom build scripts (`build.rs`)
- `[profile.release]` tuning (LTO, codegen-units, opt-level)
