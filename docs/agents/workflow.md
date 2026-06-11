# Agent Workflow

How agents should move work through this repo once the issue tracker is known.

## Source of truth

- **Issues:** GitHub Issues in `AojdevStudio/agentic-utilities`
- **Project board:** `agentic-utilities Backlog` — https://github.com/users/AojdevStudio/projects/8
- **Project owner/number:** `AojdevStudio` / `8`
- **Initial project status:** `Todo`
- **Default branch:** `main`
- **Release surface:** none configured; CI lives in `.github/workflows/ci.yml`

GitHub Issues plus the linked GitHub Project are the work hub. Keep issue state and board status there. Do not rely on local markdown task files.

## Operating loop

1. Start from the relevant issue, PRD, or user-approved spec.
2. If the work is not yet broken down, write a short plan/spec before creating implementation tickets.
3. Create or update issues in GitHub; attach new issues to the `agentic-utilities Backlog` Project.
4. Set project status to `Todo` when issues are created and `In Progress` when implementation starts.
5. Work one issue at a time unless the user explicitly asks for parallel work or the issue set is already independent.
6. Before opening or updating a pull request, run focused local checks and self-review the diff.
7. Open or update a pull request that references the issue and summarizes validation.
8. Fetch review comments with `gh`, fix comments that are valid, and reply to each resolved or rejected finding.
9. Merge and clean up branches only when the repo's merge policy allows it.
10. Update issue and project-board status before calling the work complete.

## Review conventions

- **Self-review before PR:** yes.
- **Automated review:** use whatever GitHub checks or review bots are configured on the pull request; none are required beyond CI unless the issue says otherwise.
- **Preferred review gate:** human or configured GitHub review automation.
- **Human review required before merge:** depends on the user request and repository permissions.
- **Review replies:** when a reviewer raises a point, reply with what changed or why no change was made.

## Release conventions

No release automation is configured. For release work, ask for the intended release path before creating tags, publishing packages, or changing deployment automation.
