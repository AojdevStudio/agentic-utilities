# Training Cutoff Honesty

## Principle

Model training cutoffs lag framework releases. When an agent writes code for a fast-moving framework,
it draws on training data that may be months or years behind the installed version. APIs, file
conventions, config formats, and even mental models can differ. A CLAUDE.md that explicitly tells
the agent "your training data is stale — read the local docs" prevents a class of hallucination
bugs where the model confidently invents outdated patterns.

The best single-line version of this, from a GPT-5.4 harness session on a Next.js project:

> "This is NOT the Next.js you know — APIs, conventions, and file structure may all differ from
> your training data. Read the relevant guide in `node_modules/next/dist/docs/` before writing any code."

## Detection Heuristic

After reading root `package.json`, flag projects using any of these as "fast-moving":

| Framework | Detection | Notes |
|-----------|-----------|-------|
| Next.js | `dependencies.next` or `devDependencies.next` | App Router vs Pages Router is a paradigm split |
| Tailwind CSS | `tailwindcss` in deps | v4 is a near-complete rewrite from v3 |
| React | `react` >= 18 in deps | React 19 hooks + Server Components change mental model |
| shadcn/ui | `components.json` in root OR `components/ui/` directory | Not an npm dep; detect by file presence |
| Remix / React Router v7 | `react-router` or `@remix-run/*` in deps | RR7 merged Remix; file conventions changed |
| Astro | `astro` in deps | Content Collections, view transitions, server islands |
| SvelteKit | `@sveltejs/kit` in deps | Routing and load function API changes frequently |
| Drizzle ORM | `drizzle-orm` in deps | Breaking changes ship in minor versions |
| Vite | `vite` in deps | Config API and plugin contract evolves across majors |

**Catch-all:** If a package's installed major version wasn't in widespread production use before mid-2024,
treat it as fast-moving. You can't reliably infer release date from version string alone. When in doubt,
run `npm view <pkg> time.modified` or check `node_modules/<pkg>/CHANGELOG.md` for the release date.

## Template Block

Insert this block into CLAUDE.md or AGENTS.md when fast-moving frameworks are detected. Parametrize
per project:

```markdown
## Current Versions — Read Local Docs First

This project uses <FRAMEWORK> <VERSION>, which likely includes breaking changes from
your training data. APIs, conventions, and file structure may differ. Before writing
<FRAMEWORK> code:

- Read the relevant guide in `node_modules/<PKG>/dist/docs/` (or fallback: `node_modules/<PKG>/README.md`)
- Check `CHANGELOG.md` in the package for recent deprecations
- Prefer local type definitions over training-data recall when they disagree

Applies to: <LIST OF DETECTED FAST-MOVING FRAMEWORKS>
```

## Per-Framework Doc Paths

| Framework | Primary doc path | Fallback |
|-----------|-----------------|---------|
| Next.js | `node_modules/next/dist/docs/` | `node_modules/next/README.md` + nextjs.org |
| Tailwind CSS | `node_modules/tailwindcss/` (types + README) | tailwindcss.com/docs (v4 migration guide) |
| React | `node_modules/react/` | react.dev (especially React 19 migration page) |
| shadcn/ui | `components/ui/` source + `components.json` | ui.shadcn.com/docs |
| Remix / RR7 | `node_modules/react-router/` | reactrouter.com/home |
| Astro | `node_modules/astro/` | docs.astro.build |
| SvelteKit | `node_modules/@sveltejs/kit/` | kit.svelte.dev |
| Drizzle ORM | `node_modules/drizzle-orm/README.md` | orm.drizzle.team |
| Vite | `node_modules/vite/` | vitejs.dev |

**If the expected path doesn't exist:** Still insert the block. Point to `node_modules/<pkg>/README.md`
and add a comment recommending the vendor docs site. A block that says "check the vendor docs" is
more honest than silence.
