# Publishing

## Local smoke test

```bash
npm install
npm run check
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
npm run pack:dry
```

Then publish when ready:

```bash
npm publish --access public
```
