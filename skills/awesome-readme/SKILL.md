---
name: awesome-readme
description: Creates, improves, and reviews GitHub README files using a story-first, engagement-focused structure. Use when asked to write a new README, rewrite an existing README to be more compelling, or analyze a README for clarity, differentiation, and star-worthiness.
---

# AwesomeReadme

Create GitHub READMEs that earn attention by telling a compelling story before diving into installation and reference material.

**Core philosophy:** Story before installation. Hook before documentation.

## Workflow Routing

When you execute a workflow, start with this exact notification:

```text
Running the **WorkflowName** workflow from the **AwesomeReadme** skill...
```

Route requests like this:

| Workflow | Trigger | File |
|----------|---------|------|
| **Create** | create readme, write readme, new readme | `workflows/Create.md` |
| **Improve** | improve readme, make readme better, rewrite readme, more compelling | `workflows/Improve.md` |
| **Analyze** | analyze readme, review readme, readme feedback, audit readme | `workflows/Analyze.md` |

Load the matching workflow file with the `read` tool before proceeding.

## Pi Adaptation Notes

- Use normal conversation questions instead of `AskUserQuestion`.
- If visual content is requested and no image-generation or art skill is available, produce Mermaid diagrams, ASCII diagrams, or clear image placeholders in `docs/`.
- Use the `read` tool to inspect files such as `README.md`, `package.json`, `CLAUDE.md`, and docs.
- Use `bash` only for quick directory inspection or metadata gathering.
- Preserve technical accuracy while improving structure, clarity, and emotional pull.
- Default to editing `README.md` unless the user specifies a different file.

## README Structure for Stars

The default structure should be:

1. **Hero section** - tagline, badges, one-line breakthrough
2. **The problem** - pain the reader recognizes immediately
3. **The insight** - the realization that makes the project different
4. **The solution** - short explanation of what the project is
5. **See it in action** - demo, screenshot, GIF, or code sample
6. **Features / defense** - benefit-focused table
7. **Quick start** - only after the reader is hooked
8. **How it works** - deeper technical explanation
9. **The story** - why the project exists
10. **Footer** - roadmap, contributing, license, CTA

## Hooks That Work

- Stop [doing painful thing]. Start [doing better thing].
- The first [category] that [unique benefit].
- [Things] can lie. [Tests or proof] cannot.

## What Not to Do

- Lead with installation instructions
- Bury the story at the bottom
- Open with technical jargon before emotional context
- Skip visual hierarchy
- End without a clear call to action

## References

- [Best-README-Template](https://github.com/othneildrew/Best-README-Template)
- [awesome-readme](https://github.com/matiassingers/awesome-readme)
- [shields.io](https://shields.io) for badges
- [vhs](https://github.com/charmbracelet/vhs) for terminal GIFs
