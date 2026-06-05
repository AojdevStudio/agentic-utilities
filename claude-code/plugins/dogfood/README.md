# dogfood

Systematically explore and QA a web application, then produce a report with **full reproduction evidence for every finding**.

## What it does

The skill auto-activates when you ask to dogfood, QA, exploratory-test, bug-hunt, or review the quality of a web app. Give it a target URL and it:

1. **Sets up** an output directory and seeds a structured report from a template.
2. **Opens a real browser** and (if needed) signs in, using Claude in Chrome for interactive and authenticated flows.
3. **Explores systematically** from the main navigation outward: visits each section, tests interactive elements, checks edge cases, runs realistic end-to-end workflows, and reads the console for JS errors.
4. **Documents repro-first.** When it finds an issue it stops and proves it immediately: a repro GIF plus step-by-step screenshots for interactive bugs, a single screenshot for static ones. Findings are appended to the report as they are discovered, so nothing is lost if the session is interrupted.
5. **Wraps up** with a severity summary so the report can be handed straight to the responsible team.

It aims for 5-10 well-documented issues with strong evidence rather than a long list of vague notes.

## Browser tooling

This is a Claude Code adaptation. It uses, in order of preference:

1. **Claude in Chrome** (`mcp__claude-in-chrome__*`): interactive exploration, authenticated flows, console reading, and GIF repro capture via `gif_creator`. These MCP tools load on demand (`ToolSearch`).
2. **Playwright CLI** (`bunx playwright screenshot`): a headless fallback for deterministic full-page screenshots, including in background/CI contexts.

If neither surface is available, the skill stops and says so rather than producing evidence-free findings.

## Trigger phrases

- "dogfood vercel.com"
- "QA this app"
- "exploratory test the billing page"
- "find issues / bug hunt on http://localhost:3000"
- "review the quality of this web app"

## Bundled resources

- `skills/dogfood/references/issue-taxonomy.md`: what to look for, severity levels, and the exploration checklist.
- `skills/dogfood/templates/dogfood-report-template.md`: the report scaffold copied into your output directory.

## License

MIT, see repository LICENSE.
