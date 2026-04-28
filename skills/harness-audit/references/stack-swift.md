# Swift / iOS Stack

Covers: iOS apps (UIKit + SwiftUI), watchOS, macOS apps, Swift packages. Server-side Swift (Vapor) follows similar patterns.

## Tooling matrix

| Concern | Recommended | Alternatives |
|---------|-------------|--------------|
| Lint | SwiftLint | swiftformat (just formatting) |
| Format | swift-format (Apple) or swiftformat | — |
| Test runner | `xcodebuild test` (apps) / `swift test` (SPM packages) | — |
| Pre-commit | Native git hooks via shell | Husky (works but unusual for Swift projects) |
| CI | GitHub Actions with macOS runner | Bitrise, Fastlane, Xcode Cloud |
| Project gen | XcodeGen, Tuist (optional) | Manual `.xcodeproj` (default) |

## Lint config paths to check

- `.swiftlint.yml`, `.swiftlint.yaml`
- `.swift-format` (JSON config for apple/swift-format)
- `swiftformat` config in `.swiftformat`

## Test runner one-liner

This is THE most common gap in Swift projects. `xcodebuild test` requires scheme name, destination string, derived data path — agents shouldn't compose this from memory.

`scripts/test.sh`:
```sh
#!/usr/bin/env bash
set -euo pipefail

PROJECT="MyApp.xcodeproj"
SCHEME="MyApp"
WATCH_SCHEME="MyAppWatch"  # if applicable

run_xcb() {
  if command -v xcbeautify >/dev/null 2>&1; then
    xcodebuild "$@" | xcbeautify
  else
    xcodebuild "$@"
  fi
}

case "${1:-all}" in
  ios)
    run_xcb test \
      -project "$PROJECT" -scheme "$SCHEME" \
      -destination 'platform=iOS Simulator,OS=latest,name=iPhone 15 Pro' \
      -derivedDataPath .build/DerivedData
    ;;
  watch)
    run_xcb test \
      -project "$PROJECT" -scheme "$WATCH_SCHEME" \
      -destination 'platform=watchOS Simulator,OS=latest,name=Apple Watch Series 9 (45mm)' \
      -derivedDataPath .build/DerivedData
    ;;
  all|*)
    "$0" ios
    "$0" watch  # if applicable
    ;;
esac
```

**Critical:** use `OS=latest` not pinned versions. Pinning to "iPhone 17 Pro" specifically breaks on machines with older Xcode where that simulator name doesn't exist.

## Pre-commit pattern

`.git/hooks/pre-commit`:
```sh
#!/usr/bin/env bash
set -euo pipefail

# Lint staged Swift files
if command -v swiftlint >/dev/null 2>&1; then
  STAGED=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.swift$' || true)
  if [ -n "$STAGED" ]; then
    echo "$STAGED" | xargs swiftlint lint --strict --use-script-input-files
  fi
fi

# Optional: light build check (slower, demote to pre-push if >10s)
# xcodebuild build -project MyApp.xcodeproj -scheme MyApp -quiet
```

`chmod +x .git/hooks/pre-commit` after creating.

For team-wide enforcement (since `.git/hooks/` isn't tracked), put the script in `scripts/git-hooks/pre-commit` and add a `make install-hooks` target that symlinks them.

## CI pattern (GitHub Actions, macOS)

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: macos-14  # M1 runners
    steps:
      - uses: actions/checkout@v4
      - run: xcodebuild -version  # log Xcode version
      - run: ./scripts/test.sh ios
      - run: ./scripts/test.sh watch  # if applicable
```

If the project uses SwiftUI iOS 26+ APIs (Liquid Glass, etc.), pin Xcode 26+:
```yaml
- uses: maxim-lobanov/setup-xcode@v1
  with:
    xcode-version: '26.0'
```

## Swift-specific gotchas to flag

- **AGENTS.md says "no build commands yet" but project DOES build** — the most common Swift harness failure. The original brief was scaffolded before Xcode project existed and never updated.
- **`.xcodeproj/project.pbxproj` is a binary coordination surface** — two parallel agent worktrees calling project-mutation scripts simultaneously will corrupt it. If the project uses agent worktrees, document a "one agent owns project file mutations at a time" rule.
- **Swift files created but not registered** — agents create `.swift` files but they're not in any target. Solve with `scripts/add-swift-file.rb` (xcodeproj gem) or use folder references / file system synchronized groups (Xcode 16+).
- **Strict concurrency surfacing** — `SWIFT_STRICT_CONCURRENCY = complete` in xcconfig is the right call but means agents must compile under it. Document this in rules.
- **HealthKit / device-only APIs cannot be tested in CI** — flag these for human verification on physical device. Agents should not claim "tests pass" for HealthKit changes without a device run.
- **Simulator name drift** — Xcode 15/16/17 ship different simulator catalogs. Always use `OS=latest`.

## Repo skills worth seeding

For Swift projects, common high-value repo skill entries (`.agents/skills/`, `.pi/skills/`, or `.claude/skills/` depending on the harness):

- `add-swift-file` — wraps `scripts/add-swift-file.rb`, takes file path + target list
- `check-design` — audits view code against the project's design rules
- `xcodebuildmcp` (if using MCP) — wired in `.mcp.json` for native build/test from agents
- `release-build` — wraps archive + export, handles signing

## Bonus signals (for the audit prompt)

- Workspace vs project (`.xcworkspace` vs `.xcodeproj`)
- SwiftPM dependencies (`Package.swift`) vs Cocoapods (`Podfile`) vs Carthage
- XcodeGen / Tuist usage
- `config/` directory with xcconfig files (good sign — externalized build settings)
- Strict concurrency setting in xcconfig (`SWIFT_STRICT_CONCURRENCY`)
- iOS deployment target
- Multi-target structure (iOS + watchOS + macOS)
