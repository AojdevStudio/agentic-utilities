# Package Resources

## Rules

- Keep `package.json#pi` as the source of truth for loaded Pi resources.
- Add every shared extension, skill, prompt, or theme to `docs/catalog.md` in the same change that adds the resource.
- Keep prompt templates as `prompts/*.prompt.md`; do not load documentation files as prompt commands.
- Keep runtime dependencies used by extensions in `dependencies`; keep Pi-provided packages in `peerDependencies`.
- Run `npm run check`, `npm run pack:dry`, and `pi -e .` before shipping package changes.

## Rationale

Pi discovers package resources from the manifest. Drift between files, catalog, and package metadata makes resources hard for agents and users to find.
