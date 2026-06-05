---
name: dogfood
description: "USE WHEN: dogfood, QA, exploratory test, find issues, bug hunt, test this app/site/platform, or review the quality of a web application. Systematically explores a web app in a real browser, finds bugs and UX issues, and produces a structured report with full reproduction evidence (step-by-step screenshots, repro GIFs when available, and detailed repro steps) so findings can be handed straight to the responsible team."
---

# Dogfood

Systematically explore a web application, find issues, and produce a report with full reproduction evidence for every finding.

## Browser tooling

This plugin drives a real browser through Claude Code's browser tools. Use whichever surface is available, in this order of preference:

1. **Claude in Chrome MCP tools** (`mcp__claude-in-chrome__*`): the primary surface for interactive exploration, authenticated flows, console reading, and GIF repro capture. These tools are loaded on demand. Before the first call, load them with `ToolSearch` (e.g. `select:mcp__claude-in-chrome__navigate,mcp__claude-in-chrome__computer,mcp__claude-in-chrome__read_page,mcp__claude-in-chrome__read_console_messages,mcp__claude-in-chrome__form_input,mcp__claude-in-chrome__find,mcp__claude-in-chrome__gif_creator,mcp__claude-in-chrome__tabs_context_mcp,mcp__claude-in-chrome__tabs_create_mcp`). At session start call `tabs_context_mcp` first, then open a fresh tab with `tabs_create_mcp` (do not reuse the user's existing tabs unless asked).
2. **Playwright CLI** (`bunx playwright screenshot ...`): a headless fallback for deterministic full-page screenshots when no live Chrome session is available or when you just need a static capture. It ships its own browser and runs in background/CI contexts.

If neither browser surface is available, stop and tell the user that dogfood needs a browser tool (Claude in Chrome or Playwright) and cannot proceed without one.

Use shell commands only for local filesystem work such as creating output directories, copying the report template, or moving finished artifacts. The report template lives at `${CLAUDE_PLUGIN_ROOT}/skills/dogfood/templates/dogfood-report-template.md`.

## Setup

Only the **Target URL** is required. Everything else has sensible defaults. Use them unless the user explicitly provides an override.

| Parameter | Default | Example override |
|-----------|---------|-----------------|
| **Target URL** | _(required)_ | `vercel.com`, `http://localhost:3000` |
| **Session name** | Slugified domain (e.g., `vercel.com` -> `vercel-com`) | `Session: my-session` |
| **Output directory** | `./dogfood-output/` | `Output directory: /tmp/qa` |
| **Scope** | Full app | `Focus on the billing page` |
| **Authentication** | None | `Sign in to user@example.com` |

If the user says something like "dogfood vercel.com", start immediately with defaults. Do not ask clarifying questions unless authentication is mentioned but credentials are missing.

## Workflow

```
1. Initialize    Set up output dirs, report file, browser tab
2. Authenticate  Sign in if needed
3. Orient        Navigate to starting point, take initial snapshot
4. Explore       Systematically visit pages and test features
5. Document      Capture evidence for each issue as found
6. Wrap up       Update summary counts, close the tab
```

### 1. Initialize

Create the output directories and seed the report from the template:

```bash
mkdir -p {OUTPUT_DIR}/screenshots {OUTPUT_DIR}/videos
cp "${CLAUDE_PLUGIN_ROOT}/skills/dogfood/templates/dogfood-report-template.md" {OUTPUT_DIR}/report.md
```

Fill in the report header fields (target URL, date, session name).

Open a browser tab on the target. With Claude in Chrome, call `tabs_context_mcp`, then `tabs_create_mcp` for a new tab, then `navigate` to `{TARGET_URL}` and wait for the page to settle. With the Playwright CLI alone, you will capture screenshots per page on demand instead of holding a live tab.

### 2. Authenticate

If the app requires login, use the interactive browser surface (Claude in Chrome):

1. Read the page with `read_page` (or `get_page_text`) to find the email/password fields.
2. Fill the fields with `form_input`, then click the submit control with `computer` (or locate it first with `find`).
3. Wait for navigation to settle, then confirm you are signed in.

For OTP / email codes: ask the user, wait for their response, then enter the code. Authenticated state lives in the open tab for the rest of the session; note in the report if durable auth reuse is not available on the current surface.

Playwright CLI's clean headless browser is not logged in. For authenticated targets, prefer Claude in Chrome.

### 3. Orient

Take an initial screenshot and read the page structure to map the app:

- **Claude in Chrome:** `read_page` for a structural snapshot, then a screenshot via `computer` (screenshot action). Save it to `{OUTPUT_DIR}/screenshots/initial.png`.
- **Playwright CLI:** `bunx playwright screenshot --full-page "{TARGET_URL}" {OUTPUT_DIR}/screenshots/initial.png`

Identify the main navigation elements and list the sections to visit.

### 4. Explore

Read [references/issue-taxonomy.md](references/issue-taxonomy.md) for the full list of what to look for and the exploration checklist.

**Strategy, work through the app systematically:**

- Start from the main navigation. Visit each top-level section.
- Within each section, test interactive elements: click buttons, fill forms, open dropdowns/modals.
- Check edge cases: empty states, error handling, boundary inputs.
- Try realistic end-to-end workflows (create, edit, delete flows).
- Check the browser console for errors periodically.

**At each page:**

- Read the page structure (`read_page` / `get_page_text`) to find interactive elements.
- Screenshot the page to `{OUTPUT_DIR}/screenshots/{page-name}.png` (Claude in Chrome `computer` screenshot, or `bunx playwright screenshot --full-page`).
- Read console errors and warnings with `read_console_messages` (filter with its `pattern` argument to avoid noise). Many issues are invisible in the UI but surface as JS errors or failed requests.

Use your judgment on how deep to go. Spend more time on core features and less on peripheral pages. If you find a cluster of issues in one area, investigate deeper.

### 5. Document Issues (Repro-First)

Steps 4 and 5 happen together: explore and document in a single pass. When you find an issue, stop exploring and document it immediately before moving on. Do not explore the whole app first and document later.

Every issue must be reproducible. When you find something wrong, do not just note it; prove it with evidence. The goal is that someone reading the report can see exactly what happened and replay it.

**Choose the right level of evidence for the issue:**

#### Interactive / behavioral issues (functional, ux, console errors on action)

These require user interaction to reproduce. Prefer a repro GIF plus step-by-step screenshots when recording is available.

1. **Start a repro GIF** before reproducing, if available. Claude in Chrome provides `gif_creator`: capture extra frames before and after each action so playback is smooth, and name the file meaningfully (e.g. `issue-{NNN}-repro.gif`). If no recording surface is available, rely on step-by-step screenshots and set **Repro Video** in the report to `N/A - recording unavailable in current browser surface`.

2. **Walk through the steps at human pace.** Take a screenshot at each step so the sequence is readable:

```
issue-{NNN}-step-1.png   (before)
issue-{NNN}-step-2.png   (after the action)
...
issue-{NNN}-result.png   (the broken state)
```

   Drive each action (click, type, fill) with `computer` / `form_input`, capturing a screenshot before and after.

3. **Capture the broken state** in a final screenshot so the viewer can see it clearly.

4. **Stop the GIF**, if one was started, and save it to `{OUTPUT_DIR}/videos/`.

5. Write numbered repro steps in the report, each referencing its screenshot.

#### Static / visible-on-load issues (typos, placeholder text, clipped text, misalignment, console errors on load)

These are visible without interaction. A single screenshot is sufficient. No GIF, no multi-step repro:

```bash
bunx playwright screenshot --full-page "{PAGE_URL}" {OUTPUT_DIR}/screenshots/issue-{NNN}.png
```

(or a single Claude in Chrome screenshot). Write a brief description, reference the screenshot, and set **Repro Video** to `N/A`.

---

**For all issues:**

1. **Append to the report immediately.** Do not batch issues for later. Write each one as you find it so nothing is lost if the session is interrupted.
2. **Increment the issue counter** (ISSUE-001, ISSUE-002, ...).

### 6. Wrap Up

Aim to find **5-10 well-documented issues**, then wrap up. Depth of evidence matters more than total count: 5 issues with full repro beats 20 with vague descriptions.

After exploring:

1. Re-read the report and update the summary severity counts so they match the actual issues. Every `### ISSUE-` block must be reflected in the totals.
2. Close only the tab you opened for the dogfood session (Claude in Chrome `tabs_close_mcp`). Leave the user's other tabs alone.
3. Tell the user the report is ready and summarize findings: total issues, breakdown by severity, and the most critical items.

## Guidance

- **Repro is everything.** Every issue needs proof, but match the evidence to the issue. Interactive bugs need a GIF when available plus step-by-step screenshots. Static bugs (typos, placeholder text, visual glitches visible on load) only need a single screenshot.
- **Verify reproducibility before collecting evidence.** Before recording or screenshotting, verify the issue reproduces with at least one retry. If it cannot be reproduced consistently, it is not a valid issue.
- **Don't record a GIF for static issues.** A typo or clipped text does not benefit from animation. Save recording for issues that involve user interaction, timing, or state changes.
- **For interactive issues, screenshot each step.** Capture the before, the action, and the after, so someone can see the full sequence.
- **Write repro steps that map to screenshots.** Each numbered step in the report should reference its corresponding screenshot. A reader should be able to follow the steps visually without touching a browser.
- **Use the right observation.**
  - `read_page` / `get_page_text` for finding clickable/fillable elements and reading page structure.
  - A screenshot (Claude in Chrome `computer`, or `bunx playwright screenshot`) for visual proof, layout issues, and report evidence.
  - `read_console_messages` for JS errors and failed requests.
- **Be thorough but use judgment.** You are not following a test script; you are exploring like a real user would. If something feels off, investigate.
- **Write findings incrementally.** Append each issue to the report as you discover it. If the session is interrupted, findings are preserved. Never batch all issues for the end.
- **Never delete output files.** Do not `rm` screenshots, GIFs, or the report mid-session. Do not close the session and restart. Work forward, not backward.
- **Never read the target app's source code.** You are testing as a user, not auditing code. Do not read the HTML, JS, or config files of the app under test. All findings must come from what you observe in the browser.
- **Check the console.** Many issues are invisible in the UI but show up as JS errors or failed requests.
- **Test like a user, not a robot.** Try common workflows end-to-end. Click things a real user would click. Enter realistic data.
- **Avoid blocking dialogs.** Do not trigger native JavaScript `alert`/`confirm`/`prompt` dialogs through your actions; they block the browser-automation channel. If a control may open one, warn the user before interacting with it.
- **Pace repro evidence for humans.** Leave a readable beat between actions and before the final result capture. Evidence should be understandable at normal reading speed.

## References

| Reference | When to Read |
|-----------|--------------|
| [references/issue-taxonomy.md](references/issue-taxonomy.md) | Start of session: calibrate what to look for, severity levels, exploration checklist |

## Templates

| Template | Purpose |
|----------|---------|
| [templates/dogfood-report-template.md](templates/dogfood-report-template.md) | Copy into the output directory as the report file |
