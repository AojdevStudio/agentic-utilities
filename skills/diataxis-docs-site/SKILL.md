---
name: diataxis-docs-site
description: Build, convert, and maintain source-backed Diátaxis sites with the fixed AOJ Astro Starlight starter. Use when a repository, Wiki, or docs collection must become a consistently branded site organized into tutorials, how-to guides, reference, and explanation, including publication and ongoing documentation-impact checks.
disable-model-invocation: false
---

# Diátaxis Docs Site

Build documentation around user needs, not the source material's current filenames. A conversion is a reclassification and rewrite, not a page-for-page migration.

## 1. Fix the contract

Determine from the request and repository context:

- audiences and their real tasks;
- input sources: code, GitHub Wiki, existing docs, issues, releases, and operator knowledge;
- the single canonical documentation location after publication;
- hosting, repository, URL, visibility, and deployment ownership;
- required search, versioning, localization, analytics, or private access;
- whether publication and repository changes are authorized.

Ask only when an unanswered choice changes architecture, hosting cost, access, or migration behavior.

**Complete when:** audience, canonical source, hosting boundary, and publication authority are explicit.

## 2. Establish truth

Build an evidence ledger before restructuring content:

1. Read repository and lifecycle instructions.
2. Inspect the current default-branch code, configuration, tests, CI, releases, issues, and deployed state. Prefer a code knowledge graph when available.
3. Inventory every existing documentation page and its inbound links.
4. Mark each proposed claim `verified`, `planned`, `deprecated`, or `unknown` with a source.
5. Treat an existing Wiki as a useful input, not proof that its claims remain current.

Preserve acceptance boundaries. A passing build or Simulator run is not physical-device, production, release, or deployment proof.

**Complete when:** every migrated or newly authored claim has evidence or an explicit status label, and every old page has a disposition.

## 3. Classify with the compass

Give each page one primary user need:

| Mode | User need | Form | Test |
| --- | --- | --- | --- |
| Tutorial | Learn | Guided lesson | Can a newcomer complete a safe, repeatable learning experience? |
| How-to | Accomplish | Goal-directed steps | Can a competent user reach one observable result? |
| Reference | Look up | Factual description | Does it describe the product's machinery precisely and consistently? |
| Explanation | Understand | Context and discussion | Does it answer why, connect concepts, or examine tradeoffs? |

Split pages that serve multiple modes. Link across modes instead of embedding long detours. Keep changelogs, roadmaps, support policy, and project governance outside the four modes when that is clearer than forcing a false classification.

If a classification remains ambiguous, consult the official Diátaxis definitions at <https://diataxis.fr/start-here/>.

**Complete when:** every content unit has one primary mode, mixed pages are split, and unclassified project material has an intentional home.

## 4. Design the information architecture

Use the four modes as the main documentation navigation:

- **Tutorials:** ordered learning paths with controlled prerequisites and expected observations.
- **How-to guides:** task-named pages such as “Configure…”, “Deploy…”, or “Recover…”.
- **Reference:** stable product-shaped sections for commands, configuration, APIs, schemas, statuses, and limits.
- **Explanation:** question- or concept-shaped pages for architecture, data flow, privacy, constraints, and design choices.

Add a concise home page that routes readers by intent. Preserve stable URLs where practical; otherwise create redirects for every externally or internally linked old URL.

**Complete when:** a newcomer, active user, and maintainer can each reach the right mode from Home without guessing terminology.

## 5. Instantiate the AOJ docs standard

Use the bundled `assets/aoj-starlight/` starter for every site. The fixed standard is Astro Starlight with:

- the four Diátaxis navigation groups;
- AOJ typography, spacing, color tokens, light/dark themes, and favicon;
- Markdown content under `src/content/docs/`;
- Starlight navigation, search, SEO, code highlighting, and accessibility defaults;
- a production build and GitHub Pages deployment workflow.

Create the site with `scripts/create_site.py`; do not recreate the starter by hand. Supply the project name, description, repository URL, canonical site URL, base path, and default branch. The script refuses to overwrite an existing site or workflow.

Keep the framework and AOJ design tokens fixed. Customize only the project title, description, repository link, content, and deployment URL. If GitHub Pages is unavailable, change only the hosting adapter; keep the Starlight starter and visual system unchanged. Add plugins only for confirmed needs that Starlight does not cover.

**Complete when:** the generated site contains no template tokens, installs from its lockfile, and passes `npm run check` and `npm run build` without changing the shared visual system.

## 6. Author by mode

### Tutorials

- State the learner's starting point and promised outcome.
- Control the path and provide exact steps, sample values, and expected observations.
- Keep choices and explanation minimal; link outward for alternatives and theory.
- Verify the lesson from its declared clean starting state.

### How-to guides

- Name one real goal and list prerequisites.
- Give the shortest safe route to an observable result.
- Include necessary variations, recovery, and verification without teaching the whole system.

### Reference

- Mirror the structure of the product.
- Prefer generated API or schema reference when the repository already supports it.
- Document exact names, types, defaults, constraints, errors, and compatibility.
- Link to procedures and explanations instead of mixing them into the description.

### Explanation

- Start from a specific “why” or “how does this fit together” question.
- Connect concepts, constraints, alternatives, history, and consequences.
- Keep procedures in linked how-to guides.

Across all modes, use source-backed examples, descriptive link text, useful headings, alt text, and explicit current/planned boundaries.

**Complete when:** every page passes its mode's test and no page silently changes mode midstream.

## 7. Build and validate

Run the site's native checks and add only the smallest missing check needed to cover:

- production build;
- internal links, anchors, redirects, and navigation;
- code samples or commands that can be executed safely;
- page titles, metadata, sitemap, and search indexing when present;
- keyboard navigation, heading order, contrast, and image alternatives;
- responsive layout, tables, code blocks, diagrams, and long pages;
- stale links to the prior canonical documentation location.

Preview the built site and visually inspect every template, navigation branch, and page with custom layout. Sample ordinary pages by mode; inspect every page when the site is small.

**Complete when:** the production build is green, links resolve, each mode has rendered proof, and no old canonical path strands readers.

## 8. Publish through the project lifecycle

Make site, content, redirect, CI, and deployment changes through the repository's normal issue, branch, review, and merge workflow. Use the bundled GitHub Pages workflow by default. Verify the deployed URL after merge; a green build alone is not deployment proof.

When replacing a GitHub Wiki or another canonical source:

1. Publish and verify the new site.
2. Replace the old Home page with a concise canonical-site pointer or archive it using the platform's supported mechanism.
3. Preserve redirects or pointers for known inbound links.
4. Remove duplicate editable copies once rollback needs are satisfied.

**Complete when:** the canonical URL serves the merged content and readers cannot mistake the old location for current documentation.

## 9. Install event-driven maintenance

Reuse an existing documentation-impact checkpoint when present. Otherwise add the smallest pull-request-template section:

```markdown
## Documentation impact

Select exactly one:

- [ ] No documentation update is required.
- [ ] Documentation updated — affected pages and commit: <!-- list and link -->
```

Also require concrete verification results. Map change types to modes:

| Change | Review |
| --- | --- |
| Onboarding or first-use flow | Tutorials |
| User task or recovery flow | How-to guides |
| API, configuration, schema, status, or limit | Reference |
| Architecture, privacy, constraint, or design decision | Explanation |
| Release or acceptance status | Home, relevant modes, project status pages |

Add automated documentation checks to existing CI only when they protect the build, links, generated reference, or redirects. Add no content-sync system between two canonical sources.

Make framework, dependency, navigation, and design-token changes in the bundled AOJ starter first. Validate the starter, then roll the same change across project sites; never let individual projects fork the visual system silently.

**Complete when:** every product change has one visible documentation decision and one canonical edit path.

## 10. Close with live proof

Verify and report:

- canonical site URL, repository path, framework, and deployment target;
- page inventory by Diátaxis mode;
- build, link, accessibility, and rendered inspection results;
- source commit, merged change, deployment result, and redirects;
- maintenance checkpoint and CI coverage;
- deprecated source disposition;
- planned, unknown, or externally gated work that remains unfinished.

**Complete when:** the report matches live source and hosting state, every original page is accounted for, and no migration or deployment claim rests on inference.
