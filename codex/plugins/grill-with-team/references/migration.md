# Phase 0 — First-Run Migration

Converts a brownfield repo from Matt's markdown contract (`CONTEXT.md` + `docs/adr/*.md`) to the `grill-with-team` HTML contract (`CONTEXT.html` + `docs/adr/*.html`), then deletes the markdown so there is no dual source. Destructive — gated and verified.

## When it runs

| Repo state | Action |
|------------|--------|
| `CONTEXT.html` already exists | **Skip.** Repo is already on the contract. |
| `CONTEXT.md` and/or `docs/adr/*.md` exist, no `.html` | **Migrate** (this procedure). |
| Neither markdown nor HTML | **No migration.** Create `CONTEXT.html` lazily from the template when the first term resolves. |
| Multi-context repo (`CONTEXT-MAP.md`) | Migrate the map to `CONTEXT-MAP.html` + each per-context `CONTEXT.md` → `CONTEXT.html`. |

## Procedure

1. **Gate on a clean working tree.** Run `git status --porcelain`. If dirty, stop and ask the user to commit/stash first — the migration must be its own isolated, revertable commit. If the repo isn't git-tracked, warn and ask before proceeding (no safety net).

2. **Render CONTEXT.** Read each `CONTEXT.md`. Convert every glossary term to a `<div class="term">` block (see [CONTEXT-HTML-FORMAT.md](CONTEXT-HTML-FORMAT.md)), starting from [context-template.html](context-template.html). Seed the domain map from the term relationships; seed decision-state from any decisions already captured. Pretty-print, single style block.

3. **Render ADRs.** For each `docs/adr/NNNN-slug.md`, produce `docs/adr/NNNN-slug.html` (see [ADR-HTML-FORMAT.md](ADR-HTML-FORMAT.md)), preserving number, slug, status, and content.

4. **Verify — zero content loss.** Before deleting anything, confirm:
   - Every term in the markdown glossary appears in `CONTEXT.html`.
   - Every ADR has a matching `.html`, same number/title/status.
   - No definitions were truncated or paraphrased away.
   Report the term/ADR counts (old vs new) to the user.

5. **Remove markdown.** `git rm CONTEXT.md` (and `CONTEXT-MAP.md`, `docs/adr/*.md` as applicable). Use `git rm` so the deletion is tracked.

6. **Commit alone.**
   ```
   chore(context): migrate CONTEXT + ADRs to HTML (grill-with-team)
   ```
   No other changes in this commit, so a `git revert` cleanly restores the markdown contract if needed.

## Rollback

Because the migration is one isolated commit, `git revert <sha>` restores `CONTEXT.md` + markdown ADRs exactly. The HTML files are then orphaned and can be deleted. State this to the user when migration completes.

## Notes

- Sibling consumer skills already accept either extension (`setup-matt-pocock-skills/domain.md`, `improve-codebase-architecture`), so a migrated repo keeps working with `to-prd`, `to-issues`, `triage`, `tdd`, `zoom-out`.
- Never run migration silently in the background — it deletes tracked files. Show the verification counts and let the user see the commit.
