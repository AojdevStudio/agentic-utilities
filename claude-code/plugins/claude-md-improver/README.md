# claude-md-improver

Audit and improve your `CLAUDE.md` / `AGENTS.md` instruction files so they stay a **map, not an encyclopedia**.

## What it does

The skill auto-activates when you ask to check, audit, update, improve, or fix an agent instruction file. It:

1. **Captures work-tracking context** (Phase 0): establishes which tracker and board are the project's source of truth, from repo evidence first, so the file can point agents at the real work hub instead of inventing a local task workflow.
2. **Discovers all instruction files** in the repo (root `CLAUDE.md`/`AGENTS.md`, `.claude.local.md`, monorepo subsystem files) and reads `package.json` to detect fast-moving frameworks.
3. **Scores each file** on a 100-point rubric across five criteria: project intent & conventions, anti-staleness, leanness, non-obvious gotchas, and discoverability.
4. **Outputs a quality report first**, always, before touching anything: grades, staleness risks, missing signal, and recommended cuts.
5. **Applies targeted edits after approval** using three operations: **Cut** (remove discoverable/volatile content), **Point** (redirect to source of truth), and **Add** (inject knowledge agents can't infer).
6. **Recommends training-cutoff-honesty blocks** for projects on fast-moving frameworks (Next.js, Tailwind v4, React 19, shadcn/ui, Drizzle, Vite) so agents read local package docs instead of inventing outdated APIs.

## Trigger phrases

- "audit my CLAUDE.md"
- "improve my AGENTS.md"
- "check my agent instructions"
- "is my CLAUDE.md too long / stale?"
- "CLAUDE.md maintenance"
- "project memory optimization"

## Philosophy

Context is a scarce resource. Every token in an instruction file competes with the task, the code, and the docs. The ideal file is ~100-150 lines that teach agents *how the project thinks*, with pointers to deeper sources of truth, not a catalog of things agents can already discover.

## References

The skill bundles four reference files it loads on demand:

- `references/quality-criteria.md`: the full scoring rubric and red/green flags.
- `references/templates.md`: lean instruction-file templates by project type.
- `references/update-guidelines.md`: the cut/point/add operations in detail.
- `references/training-cutoff-honesty.md`: framework detection heuristics and template blocks.

## License

MIT, see repository LICENSE.
