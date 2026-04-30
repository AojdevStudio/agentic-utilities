# Output Path Resolution

Used by Phase 4 Step 4.5 when `mode == "document"`. Skip this entirely when `mode == "chat"`.

## Resolving the output directory

The user picks where document-mode analyses are saved. The skill checks for a configured output directory in this order:

### 1. Plugin settings file (preferred)

Look for an output directory configured in the project's plugin settings file:

```
${CLAUDE_PROJECT_DIR}/.claude/youtube-analyzer.local.md
```

Example contents:

```markdown
---
output_directory: /Users/me/notes/research
---
```

If the file exists and `output_directory` is set, use that value.

### 2. Ask the user (first run)

If no settings file exists, prompt the user with `AskUserQuestion`:

```json
{
  "questions": [{
    "question": "Where should saved analyses be written? (You can change this later by editing .claude/youtube-analyzer.local.md.)",
    "header": "Output dir",
    "options": [
      {"label": "Current project directory", "description": "Save to ${CLAUDE_PROJECT_DIR}/youtube-analyses/"},
      {"label": "Subdirectory of cwd", "description": "I'll specify a subdirectory under ${CLAUDE_PROJECT_DIR}/"},
      {"label": "Absolute path", "description": "I'll provide a full absolute path (e.g., /Users/me/notes/research)"}
    ],
    "multiSelect": false
  }]
}
```

After the user answers, write the resolved absolute path into `.claude/youtube-analyzer.local.md` so the prompt only fires once per project.

### 3. Fallback

If both the settings file and the prompt fail (rare — `AskUserQuestion` declined or unavailable), default to `${CLAUDE_PROJECT_DIR}/youtube-analyses/` and create the directory if missing.

## Filename format

- Pattern: `{YYYY-MM-DD}-{descriptive-name}.md`
- Date: today's date in ISO format
- Descriptive name: kebab-case, 5–8 words max, drawn from video title or topic
- Example: `2026-04-27-dividend-portfolio-strategy.md`

## Optional: Copy to current working directory

After the canonical file is written, Step 4.7 in `SKILL.md` prompts the user about copying the same file to the cwd captured at skill invocation. This is purely additive — the resolved output directory above is always the primary location.

## Why no hard-coded path?

The personal (PAI) version of this skill wrote every analysis to a single explicit research-vault path under the author's home directory. That path was removed for the public plugin so each user can pick their own destination once and reuse it across runs.
