# Go Stack

Covers: Go modules, CLIs, services, workers, monorepos with multiple Go modules.

## Tooling matrix

| Concern | Recommended | Alternatives |
| --- | --- | --- |
| Format | `gofmt` / `go fmt` | `gofumpt` |
| Lint | `golangci-lint` | `go vet`, staticcheck |
| Test runner | `go test ./...` | gotestsum, richgo |
| Pre-commit | pre-commit framework or tracked hook installer | lefthook |
| CI | GitHub Actions | Buildkite, CircleCI |

## Config paths to check

- `go.mod`, `go.sum`
- `go.work`, `go.work.sum`
- `.golangci.yml`, `.golangci.yaml`, `.golangci.toml`, `.golangci.json`
- `Makefile`, `Taskfile.yml`, `justfile`
- `.github/workflows/*`

## Pre-commit pattern

Prefer tracked hook scripts plus an installer, or the pre-commit framework. Do not rely on writing only to `.git/hooks/`; that directory is not version-controlled.

Example tracked hook:

`scripts/git-hooks/pre-commit`:
```sh
#!/usr/bin/env bash
set -euo pipefail

STAGED_GO=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.go$' || true)
if [ -z "$STAGED_GO" ]; then
  exit 0
fi

# Format staged Go files and restage them.
echo "$STAGED_GO" | xargs gofmt -w
echo "$STAGED_GO" | xargs git add

go vet ./...
if command -v golangci-lint >/dev/null 2>&1; then
  golangci-lint run ./...
fi
```

`Makefile`:
```make
install-hooks:
	mkdir -p .git/hooks
	ln -sf ../../scripts/git-hooks/pre-commit .git/hooks/pre-commit
```

## Test wrapper

`go test ./...` is the baseline one-liner. For agent use, expose a stable command:

```make
verify:
	go test ./...
	go vet ./...
	golangci-lint run ./...
```

For slow integration tests, use build tags:

```sh
go test ./...                  # fast/unit
go test -tags=integration ./... # integration
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
      - uses: actions/setup-go@v5
        with:
          go-version-file: go.mod
          cache: true
      - run: go test ./...
      - run: go vet ./...
      - uses: golangci/golangci-lint-action@v6
```

## Go-specific gotchas to flag

- **Context propagation** — services should pass `context.Context`; flag long-running calls without timeout/deadline.
- **Ignored errors** — `errcheck` or `golangci-lint` should catch these.
- **Global mutable state** — parallel tests and agent edits often expose races; recommend `go test -race ./...` for critical packages.
- **Integration tests hitting real services** — require env-gated tests and `.env.example` docs.
- **Generated code drift** — check `go generate ./...` or documented generation commands.
- **Multiple modules** — `go test ./...` from one module will not cover sibling modules unless `go.work` or a wrapper exists.

## Repo skills worth seeding

- `run-go-tests` — wraps fast/unit/integration/race variants
- `regen-go-code` — wraps `go generate ./...` and protobuf/sqlc generation
- `inspect-ci-failure` — summarizes failing Go test output from CI

## Bonus signals (for the audit prompt)

- Go version (`go` directive in `go.mod`)
- Single module vs `go.work` workspace
- Race test coverage for concurrent code
- Presence of generated code (`// Code generated`, `sqlc`, protobuf, mockgen)
- Service framework hints (gin, chi, echo, connect-go, grpc)
