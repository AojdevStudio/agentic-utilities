# Publishing

## Local smoke test

```bash
bun install --no-save --ignore-scripts
bun run check
pi -e .
```

Inside Pi, verify:

```text
/agentic-utilities
```

Ask the agent to call `agentic_utilities_ping` as a tool if needed.

## Git install

After pushing to GitHub:

```bash
pi install git:github.com/<user>/agentic-utilities
```

Pin releases with tags:

```bash
git tag v0.1.0
git push origin v0.1.0
pi install git:github.com/<user>/agentic-utilities@v0.1.0
```

## npm publishing

The repo is npm-package-ready because `package.json` includes:

- `keywords: ["pi-package"]` for Pi package gallery discoverability.
- `files` allowlist to avoid shipping local junk.
- `peerDependencies` for Pi-provided packages.

Dry run before publish:

```bash
bun run pack:dry
```

Then publish when ready:

```bash
bun publish --access public
```

## Standalone stars package

`stars/` is an independent npm package, separate from the root Pi package. The root `package-lock.json` and published-files allowlist remain the Pi package contracts.

From the repository root, run `bun run check` and `bun run pack:dry`. Then validate the CLI package and inspect its own archive:

```bash
bun test stars/test/
cd stars
bun pm pack --dry-run
bun pm pack
```

Inspect the generated tarball and install it into a temporary Bun home with a temporary `HOME` and `STARS_DATA_DIR`. Run `stars --help`, `stars --version`, and mocked `stars star OWNER/REPO --json` from outside this checkout. Verify the archive contains only the package manifest and files in `stars/package.json#files`.

Before publishing, verify the npm registry URL, authenticated registry identity, permission to publish `aoj-stars`, package-name availability, and the intended version. The GitHub organization does not establish npm scope ownership. The release command from `stars/` is:

```bash
bun publish --access public
```

If the registry asks for a one-time password, use Bun's `--otp` option through the normal interactive release process. Confirm the published version in the registry and install it in a fresh environment with `bun add --global aoj-stars@0.1.0`. Verify the installed `stars` executable resolves on PATH and runs outside this repository. A packed or linked checkout is only a local validation artifact.

The CLI needs Bun 1.2 or newer and authenticated `gh` on macOS or Linux. The [package README](../stars/README.md) documents state migration and optional integrations. The public [stars skill](../skills/stars/SKILL.md) is generated from the canonical skill store through `public-manifest.json`; update the source and run the one-way sync before release.
