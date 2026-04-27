# Repository Instructions

This repo is a Pi package for reusable Pi extensions, Agent Skills, prompt templates, and themes.

## Rules

- Keep resources discoverable through `package.json#pi`.
- Do not commit secrets, API keys, credentials, session files, or machine-local config.
- Extension names and skill names use kebab-case directories.
- Tool names inside extensions should use snake_case and be globally specific, e.g. `agentic_utilities_ping`.
- Skills must follow Agent Skills structure: `skills/<name>/SKILL.md`, frontmatter `name` matching the directory, and a precise `description`.
- For extension string enum parameters, use `StringEnum` from `@mariozechner/pi-ai`, not `Type.Union([Type.Literal(...)])`.
- Custom tools that mutate files must use `withFileMutationQueue()` around the full read-modify-write window.
- Custom tools with large output must truncate output and report where the full output is stored.
- Keep each resource documented in `docs/catalog.md`.

## Verification

Run before shipping changes:

```bash
npm run check
npm run pack:dry
pi -e .
```

<!-- opensrc:start -->

## Source Code Reference

Source code for dependencies is available in `opensrc/` for deeper understanding of implementation details.

See `opensrc/sources.json` for the list of available packages and their versions.

Use this source code when you need to understand how a package works internally, not just its types/interface.

### Fetching Additional Source Code

To fetch source code for a package or repository you need to understand, run:

```bash
npx opensrc <package>           # npm package (e.g., npx opensrc zod)
npx opensrc pypi:<package>      # Python package (e.g., npx opensrc pypi:requests)
npx opensrc crates:<package>    # Rust crate (e.g., npx opensrc crates:serde)
npx opensrc <owner>/<repo>      # GitHub repo (e.g., npx opensrc vercel/ai)
```

<!-- opensrc:end -->