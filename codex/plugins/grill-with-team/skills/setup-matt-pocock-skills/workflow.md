# Agent Workflow

How agents should move work through this repo once the issue tracker is known.

## Source of truth

- **Issues:** [GitHub Issues / GitLab Issues / local markdown / other]
- **Project board:** [GitHub Project owner/number/title/URL / local board]
- **Default branch:** [main / master / other]
- **Release surface:** [.github/workflows/release.yml / manual / none configured]

If this repo is GitHub-backed, GitHub Issues plus the linked GitHub Project are
the default work hub. Keep status there. Do not rely on local markdown task
files unless this doc explicitly says local markdown is the source of truth.

## Operating loop

1. Start from the relevant issue, PRD, or user-approved spec.
2. If the work is not yet broken down, write a short plan/spec before creating implementation tickets.
3. Create or update issues in the configured tracker; attach them to the project board when one is configured.
4. Set project status to `Todo` when issues are created and `In Progress` when implementation starts.
5. Work one issue at a time unless the user explicitly asks for parallel work or the issue set is already independent.
6. Before opening or updating a PR, run the focused local checks and self-review the diff.
7. Open or update a PR that references the issue and summarizes validation.
8. Fetch review comments with the configured tracker CLI, fix comments that are valid, and reply to each resolved or rejected finding.
9. Merge and clean up branches only when the repo's merge policy allows it.
10. Update issue and project-board status before calling the work complete.

## Review conventions

- **Self-review before PR:** [yes / no]
- **Automated review:** [Codex / CodeRabbit / GitHub Copilot / none configured]
- **Preferred review gate:** [Pullsmith / CodeRabbit / Greptile / Codex / human / repo-specific]
- **Human review required before merge:** [yes / no / depends]
- **Review replies:** When a reviewer raises a point, reply with what changed or why no change was made.

## Release conventions

If the repo has a release or deploy workflow, treat it as part of the agent operating surface. For release work, prefer the existing workflow and document the exact command or tag pattern here.

- **Trigger:** [tag push / workflow_dispatch / manual / none]
- **Artifacts:** [archives / binaries / packages / none]
- **Release notes:** [generated / manual / none]

Release and deploy workflows are part of the agent operating surface. If a repo
ships artifacts, document how agents build them, name them, generate release
notes, and create the GitHub release. Do not create or modify release
automation unless the user asks for release work or the current issue explicitly
requires it.
