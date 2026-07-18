---
name: github-wiki
description: Build and maintain a canonical, source-backed GitHub Wiki. Use when a project needs a new Wiki, a complete Wiki refresh, rendered-page verification, or an event-driven process that keeps Wiki content aligned with code, issues, and releases.
---

# GitHub Wiki

Build the Wiki as verified product documentation, not marketing copy. Treat merged code and live repository state as truth; label everything else explicitly.

## 1. Fix the contract

Determine from the request and repository context:

- audience: users, developers, operators, or a mix;
- canonical location: GitHub Wiki alone or an existing documented alternative;
- truth boundary: merged behavior, released behavior, Simulator proof, physical-device proof, and planned work;
- publication scope: draft locally, publish directly, or publish plus repository maintenance changes;
- maintenance model: event-driven updates by default.

Ask only about choices that materially change the result. Use explicit user authorization to publish when already given.

**Complete when:** the audience, source of truth, publication boundary, and acceptance boundary are unambiguous.

## 2. Build an evidence ledger

Inspect before writing:

1. Read repository instructions and current lifecycle rules.
2. Inspect the architecture, entry points, configuration, tests, CI, release setup, and user-visible flows. Prefer a code knowledge graph when available.
3. Inspect live GitHub issues, pull requests, releases, default branch, visibility, and Wiki state.
4. Record each proposed factual claim with its source and status: `verified`, `planned`, or `unknown`.
5. Resolve conflicts in favor of current code and live GitHub state. Preserve unresolved conflicts as explicit unknowns.

Never present a passing build, Simulator run, or open issue as physical-device, TestFlight, production, or release proof.

**Complete when:** every product, architecture, privacy, setup, testing, and release claim planned for publication has evidence or an explicit status label.

## 3. Design the smallest complete page set

Create only pages supported by the project. A mature application usually needs:

- Home
- Getting Started
- User Guide
- Architecture
- Data Flow and Durability
- Privacy
- Development
- Testing and Simulator
- Troubleshooting
- Releases and TestFlight
- Roadmap
- FAQ
- Wiki Maintenance
- `_Sidebar.md` and `_Footer.md`

Collapse or omit pages with no real content. Add domain pages only when the code or operating model requires them. Keep GitHub Wiki canonical; do not create a mirrored `docs/wiki` tree.

**Complete when:** every target audience has a clear path from Home to setup, use, understanding, recovery, and current status without placeholder pages.

## 4. Author from sources

Write concise Markdown with:

- one H1 per content page;
- relative Wiki links such as `[[Getting Started]]`;
- exact commands and configuration names verified against the repository;
- diagrams only where relationships are materially clearer than prose;
- links to live issues for unfinished acceptance or roadmap work;
- explicit local, cloud, privacy, retention, and deletion boundaries;
- recovery steps tied to actual failure states;
- a current-status section that separates completed, pending, and planned work.

Do not infer features from intent, naming, mocks, or unmerged work.

**Complete when:** every page is useful on its own, internally consistent, and traceable to the evidence ledger.

## 5. Initialize and publish safely

GitHub creates the separate `<owner>/<repo>.wiki.git` repository only after the first Wiki page exists. If the Wiki is empty:

1. Create the initial Home page through the authenticated GitHub UI.
2. Clone `<repository-url-without-.git>.wiki.git` into a separate working directory.
3. Add the complete page set.
4. Validate before committing.
5. Publish the Wiki as one atomic commit unless an established Wiki history requires another convention.

Keep application changes out of the Wiki repository. Preserve unrelated working-tree changes.

**Complete when:** the Wiki remote contains the intended atomic commit and the local Wiki checkout is clean and synchronized.

## 6. Validate source and rendered output

Before publication, verify:

- every internal Wiki link resolves to a page;
- filenames and link slugs agree;
- each content page has one H1;
- code fences and tables are balanced;
- Markdown has no whitespace errors;
- the page inventory matches `_Sidebar.md`;
- no stale placeholders or unsupported claims remain.

After publication, open every page on GitHub and visually inspect:

- canonical URL and expected heading;
- sidebar and footer;
- internal navigation;
- tables, code blocks, images, and diagrams;
- narrow or long pages for clipping and broken layout.

Wait for asynchronous diagram rendering before judging it. Repair and republish any failure, then repeat the affected checks.

**Complete when:** every published page has passed both source validation and rendered GitHub inspection.

## 7. Install event-driven maintenance

Add a Wiki Maintenance page that maps change types to affected pages. Include at least:

| Change | Review these pages |
| --- | --- |
| User flow or setup | Getting Started, User Guide, Troubleshooting, FAQ |
| Architecture or storage | Architecture, Data Flow and Durability, Privacy |
| Testing or acceptance status | Testing and Simulator, Roadmap, Home |
| Release or distribution | Releases and TestFlight, Getting Started, Home |
| Closed roadmap issue | Roadmap and every page named by that issue |

If the application repository lacks an equivalent checkpoint, add the smallest pull-request-template section:

```markdown
## Wiki impact

Select exactly one:

- [ ] No Wiki update is required.
- [ ] Wiki updated — affected pages and Wiki commit: <!-- list pages and link the Wiki commit -->
```

Also require concrete verification commands or checks with results. Make repository changes through its normal issue, branch, CI, review, and merge workflow. Add no Wiki automation until missed updates demonstrate that the checkpoint is insufficient.

**Complete when:** future changes have one visible decision point, one canonical Wiki, and an owner-visible path to update affected pages.

## 8. Close with live proof

Verify and report:

- Wiki URL, page count, and Wiki commit;
- rendered inspection result;
- maintenance mechanism and merged repository change, if any;
- CI and review result;
- open acceptance work that remains explicitly unfinished;
- repository visibility, default branch, and clean synchronization state when relevant.

**Complete when:** the reported state matches live GitHub state and no required work is hidden behind vague wording.
