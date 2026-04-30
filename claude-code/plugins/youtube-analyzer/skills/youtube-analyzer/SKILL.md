---
name: youtube-analyzer
description: Analyze YouTube videos with content-type detection, multi-agent transcript chunking, optional GitHub repo exploration, and structured markdown output. Use when the user asks to "analyze a YouTube video", "summarize this video", "extract insights from a tutorial", or pastes a YouTube URL with intent to analyze.
---

# YouTubeAnalyzer Skill

**Purpose:** Comprehensive YouTube video analysis with content-type detection, specialized format workflows (tutorial/finance/general/etc.), multi-agent orchestration for long videos, and optional GitHub repo cross-referencing for tutorial content.

**Architecture:** This file is the orchestration spine — 4 phases with blocking gates. Mechanical detail is progressively disclosed via `references/*.md` (loaded by phase) and content-analysis rules live in `workflows/*.md` (passed into sub-agents).

| File | Read when |
|------|-----------|
| `references/source-selection.md` | Starting Phase 1 |
| `references/scaling-and-repo-explore.md` | Starting Phase 3 |
| `references/output-paths.md` | Phase 4 Step 4.5, only if `mode == "document"` |
| `references/output-templates.md` | Phase 4 Step 4.3, passed into synthesis agent |
| `content-types.md` | Phase 2 Q1, when detection confidence < 60% |
| `workflows/<format>-workflow.md` | Phase 4 Step 4.1, passed into chunk agents |
| `package-database-schema.md` | Phase 4 Step 4.4, tutorials only |

---

## Invocation Flags

Parse the user's invocation BEFORE Phase 1. Flags pre-set Phase 2 outputs and suppress the matching prompt.

**Syntax accepted** (any of these forms):
- `--chat <url>` — deliver inline, no file written
- `--chat: <url>` / `--chat=<url>`
- `--document <url>` (explicit; default behavior)

| Flag | Effect | Pre-sets |
|------|--------|----------|
| `--chat` | Skip Q5; deliver analysis inline (no file written) | `mode = "chat"` |
| `--document` | Skip Q5; save to resolved output directory (default) | `mode = "document"` |

**Parsing rules:**
1. Strip the flag token (and any trailing `:` or `=value`) before extracting the URL
2. If a flag was passed: set `mode` and SKIP Q5
3. Both flags present → last one wins
4. Unknown flags (`--foo`) → warn user inline, proceed without it

When a flag was detected, surface it: *"Flag detected: `--chat` → mode preset to chat (Q5 skipped)."*

---

## Required External Tools

Verify these are installed before Phase 1. If missing, surface a single install message and stop:

- **`yt-dlp`** — `pip install yt-dlp` or `brew install yt-dlp`
- **`youtube_transcript_api`** — `pip install youtube-transcript-api`
- **`bun`** — `curl -fsSL https://bun.sh/install | bash`

```bash
command -v yt-dlp && command -v youtube_transcript_api && command -v bun
```

---

## Capture invocation cwd

BEFORE Phase 1, capture the working directory the user invoked the skill from. Store as `invocationCwd`. Used by Step 4.7 (post-save copy prompt).

```bash
pwd
```

Persist that value through all phases. Sub-agents may run elsewhere — only the orchestrator's initial cwd counts.

---

## Runtime Requirements

This skill runs from any agent context (the primary thread, or a delegated agent like `general-purpose`) provided the calling agent has: `Agent`/`Task`, `AskUserQuestion`, `Write`/`Edit`, `Bash`, `Read`, `Grep`. Agents lacking any of these (e.g. `Explore`) should defer — auto-detect via tool availability, not by name.

The terms "the orchestrator" and "the calling agent" both refer to whatever agent is executing this skill.

---

## Orchestration Model

**The orchestrator NEVER does analysis work directly.** All content extraction, processing, and writing is delegated to specialized sub-agents via the Task tool. The orchestrator coordinates, routes, and writes files (in `mode == "document"`) once results return.

**EVERY phase has a BLOCKING GATE.** Do not proceed until the gate checklist is satisfied.

---

## PHASE 1: SOURCE SELECTION

### BLOCKING GATE 1

```
PRE-CONDITIONS: External tools verified
MANDATORY OUTPUTS:
  - transcriptPath: string     # Path to loaded clean transcript file
  - transcriptSource: string   # "yt-dlp" | "youtube_transcript_api"
  - videoMetadata: object      # { title, channel, duration?, upload_date?, video_id?, topic? }
  - wordCount: number          # Estimated word count
  - transcriptQuality: string  # "HIGH" | "MEDIUM" | "NONE" | "UNAVAILABLE"
```

**Mechanics:** Read `references/source-selection.md`. It covers URL extraction, the 4-tier transcript fallback chain, and VTT cleanup.

**Auto-detect:** If the user already provided a YouTube URL, skip the URL prompt and go directly to metadata extraction.

**Gate 1 checklist (verify ALL):**
- [ ] `transcriptPath` exists and is readable
- [ ] `transcriptSource` is set
- [ ] `videoMetadata.title` and `videoMetadata.channel` are non-empty
- [ ] `wordCount > 0`
- [ ] `transcriptQuality` is set

> "Phase 1 complete. {wordCount} words loaded from {transcriptSource}. Proceeding to config..."

---

## PHASE 2: INTERACTIVE CONFIG

### BLOCKING GATE 2

```
PRE-CONDITIONS:
  - transcriptPath exists and is readable
  - videoMetadata is populated (title + channel at minimum)
MANDATORY OUTPUTS:
  - category: string            # business, finance, technology, etc.
  - format: string              # tutorial | course | finance | interview | lecture | general
  - outputSelection: string[]   # Selected output types
  - depth: string               # "quick" | "standard" | "deep"
  - focusArea: string           # Format-specific focus (or "none")
  - repoUrl: string | null      # GitHub repo URL (tutorials only)
  - confidence: number          # Detection confidence percentage
  - mode: string                # "document" | "chat" — see Q5 (or pre-set by flag)
```

### Q1: Content Type Confirmation

**Detection:** Run the content-type detector with metadata extracted in Phase 1:

```bash
bun run ${CLAUDE_PLUGIN_ROOT}/scripts/detect-content-type.ts --url "URL"
# OR pipe metadata JSON
echo '{"title":"...","description":"...","tags":[...]}' | bun run ${CLAUDE_PLUGIN_ROOT}/scripts/detect-content-type.ts --json
```

If confidence < 60%, present alternatives. See `content-types.md` for full keyword lists and scoring.

```json
{
  "questions": [{
    "question": "I detected this as {category}/{format} ({X}% confidence). Is that correct?",
    "header": "Content type",
    "options": [
      {"label": "Yes, proceed", "description": "Use {category}/{format} as detected"},
      {"label": "Change category", "description": "Keep {format} format, pick a different category"},
      {"label": "Change format", "description": "Keep {category} category, pick a different format"},
      {"label": "Change both", "description": "Select both category and format manually"}
    ],
    "multiSelect": false
  }]
}
```

### Q2: Output Selection (multiSelect: true)

The available options depend on format:

**tutorial / course:** Detailed Summary · Production Checklist · Tool/Package Inventory · Key Quotes
**finance:** Strategy Breakdown · Action Items · Risk Analysis · Key Quotes
**general / interview / lecture:** Detailed Summary · Deep Analysis · Key Insights · Key Quotes

**Default if user says "all" or doesn't specify:** Detailed Summary + Key Quotes.

### Q3: Output Depth

`A) Quick (~500w)` · `B) Standard (~1500w, recommended)` · `C) Deep dive (~3000w+)`

### Q4: Format-Specific

- **Tutorial / Course:** "GitHub repo for this tutorial? Paste URL or skip." (stored as `repoUrl`)
- **Finance:** "Actionable takeaways or theoretical analysis?"
- **General / Interview / Lecture:** "Any specific angle to emphasize?"

### Q5: Delivery Mode

**SKIP this question entirely if `mode` was pre-set by an invocation flag.** Otherwise:

```json
{
  "questions": [{
    "question": "How should the analysis be delivered?",
    "header": "Delivery",
    "options": [
      {"label": "Save to disk", "description": "Default — write a permanent markdown file at the resolved output directory. Best when you want to reference this later."},
      {"label": "Discuss in chat", "description": "Run the full analysis but return the rendered markdown inline so we can talk through it. No file is created."}
    ],
    "multiSelect": false
  }]
}
```

Map: `Save to disk` → `mode = "document"`; `Discuss in chat` → `mode = "chat"`. **Default:** `mode = "document"`.

### Gate 2 checklist (verify ALL):
- [ ] `category` is a valid category from `content-types.md`
- [ ] `format` is one of: tutorial, course, finance, interview, lecture, general
- [ ] `outputSelection` has ≥1 item
- [ ] `depth` is set
- [ ] `focusArea` is set (can be "none")
- [ ] `repoUrl` is a string or null
- [ ] `mode` is "document" or "chat"

> "Phase 2 complete. {category}/{format} at {confidence}% confidence. Depth: {depth}. Outputs: {outputSelection}. Mode: {mode}. Proceeding to scaling..."

---

## PHASE 3: MULTI-AGENT SCALING + REPO EXPLORATION

### BLOCKING GATE 3

```
PRE-CONDITIONS:
  - category, format, outputSelection, depth all set
  - transcriptPath exists
MANDATORY OUTPUTS:
  - partitionStrategy: string         # "single" | "multi"
  - agentCount: number                # 1 for single, 2+ for multi
  - chunkPaths: string[]              # Chunk file paths
  - repoExploreResults: object | null # Mermaid diagrams from repo exploration
```

**Mechanics:** Read `references/scaling-and-repo-explore.md`. It covers token estimation, the scaling decision table (≤30K / 30–100K / >100K), partition execution, and the 3-explorer parallel repo exploration (StructureExplorer, DependencyExplorer, PatternExplorer — all `subagent_type: Explore`).

**Trigger for repo exploration:** `repoUrl` is non-null AND format is `tutorial` or `course`. Run in parallel with transcript partitioning.

### Gate 3 checklist (verify ALL):
- [ ] `partitionStrategy` is set
- [ ] `agentCount ≥ 1`
- [ ] `chunkPaths` has ≥1 path; each path exists and is readable
- [ ] `repoExploreResults` is set (object or null)
- [ ] If `repoUrl` was provided, either `repoExploreResults` has content OR a skip reason is documented

> "Phase 3 complete. {agentCount} agent(s) ready. {repoExploreResults ? 'Repo exploration complete with Mermaid diagrams.' : ''} Dispatching {format} workflow..."

---

## PHASE 4: WORKFLOW DISPATCH + SYNTHESIS

### BLOCKING GATE 4

```
PRE-CONDITIONS:
  - All Gate 3 outputs satisfied
  - category, format, outputSelection, depth, focusArea, mode all set
MANDATORY OUTPUTS:
  - workflowLoaded: string                # Workflow file that was loaded
  - outputPath: string | null             # Canonical output path (null when mode == "chat")
  - renderedMarkdown: string | null       # Rendered markdown (populated when mode == "chat")
  - copyPath: string | null               # Optional cwd copy path from Step 4.7 (null if declined or chat mode)
```

**Final-phase reporting (branch on mode):**
> If `mode == "document"` AND `copyPath` is set: "Analysis complete. Saved to {outputPath}. Copy also at {copyPath}."
> If `mode == "document"` AND `copyPath` is null: "Analysis complete. Saved to {outputPath}."
> If `mode == "chat"`: "Analysis complete — rendered below for discussion. No file written." Then print `renderedMarkdown` inline so it enters the conversation context.

---

### Step 4.1: Dispatch

**Dispatch table:**

| Format | Workflow File | Agent Description | Agent Type | Model |
|--------|---------------|-------------------|------------|-------|
| tutorial | `workflows/tutorial-workflow.md` | TutorialAgent | `general-purpose` | `sonnet` |
| course | `workflows/tutorial-workflow.md` | TutorialAgent | `general-purpose` | `sonnet` |
| finance | `workflows/finance-workflow.md` | FinanceAgent | `general-purpose` | `sonnet` |
| interview | `workflows/general-workflow.md` | GeneralAgent | `general-purpose` | `sonnet` |
| lecture | `workflows/general-workflow.md` | GeneralAgent | `general-purpose` | `sonnet` |
| general | `workflows/general-workflow.md` | GeneralAgent | `general-purpose` | `sonnet` |

**Why `general-purpose` on `sonnet`:** Workflow agents follow precise file instructions to extract structured data and produce markdown. `general-purpose` executes workflow instructions directly without loading specialized agent personalities.

**Pre-dispatch verification:**
1. Format maps to a valid workflow file in the table above
2. Read the workflow file contents
3. Agent prompt includes the full workflow file
4. Agent receives: chunk path + user config + video metadata
5. `subagent_type: "general-purpose"` and `model: "sonnet"` set explicitly

### Step 4.2: Launch Workflow Agents

Per chunk, launch a Task agent with `subagent_type: "general-purpose"`, `model: "sonnet"`. Each agent receives:
1. Its chunk file path (or full transcript if single agent)
2. User config: `{ category, format, outputSelection, depth, focusArea }`
3. Video metadata: `{ title, channel, duration, upload_date, video_id }`
4. Workflow instructions (full content of the appropriate `workflows/*.md`)
5. Instruction: "Analyze ONLY your assigned chunk. Do not read beyond your assigned content."

**Parallel:** if `agentCount > 1`, launch ALL chunk agents in parallel (single message, multiple Task calls).

### Step 4.3: Synthesis

Launch ONE synthesis agent (`general-purpose`, `sonnet`) with a fresh context. Pass it `references/output-templates.md` (YAML frontmatter, Production Checklist section, Ground Truth Architecture section, Package Version table) so it can apply the templates without polluting the orchestrator context.

**Synthesis agent receives:**
1. Merged chunk analysis results
2. User config (including `mode`)
3. Video metadata
4. `repoExploreResults` if non-null (Mermaid diagrams)
5. Target output path — only if `mode == "document"`; pass `null` for chat
6. Contents of `references/output-templates.md`

**Synthesis agent does:**
1. Merge + deduplicate chunk analyses
2. Apply output template based on `format` + `outputSelection`
3. Generate YAML frontmatter
4. If tutorial + repoExploreResults: add Ground Truth Architecture section with Mermaid diagrams
5. Tutorials: extract package list for the package database (always — runs regardless of mode)
6. **Branch on mode:**
   - `mode == "document"` → write file to output path. Return `{ outputPath, renderedMarkdown: null, wordCount, packagesFound[] }`
   - `mode == "chat"` → skip file write. Return `{ outputPath: null, renderedMarkdown: <full markdown including frontmatter>, wordCount, packagesFound[] }`. Orchestrator prints `renderedMarkdown` inline.

### Step 4.4: Package DB Integration (Tutorials Only)

For each package found:

```bash
bun run ${CLAUDE_PLUGIN_ROOT}/scripts/package-db.ts add \
  --name "{packageName}" --display-name "{displayName}" \
  --version-mentioned "{version}" --category "{packageCategory}" \
  --source "{videoUrl}"
```

The package database lives at `~/.config/youtube-analyzer/package-db.json` (created on first use).

**Version lookup** (post-analysis, batches of 5; skip packages checked within 7 days):
- npm: `WebFetch` on `https://registry.npmjs.org/{pkg}/latest`
- PyPI: `WebFetch` on `https://pypi.org/pypi/{pkg}/json`
- Other: `WebSearch`

Update the package database with `--latest-version`. Status thresholds and the Package Version table format live in `references/output-templates.md`. Schema details: `package-database-schema.md`.

### Step 4.5: Output Path Resolution

**Skip this step entirely when `mode == "chat"`** — set `outputPath = null` and continue to cleanup.

For `mode == "document"`: read `references/output-paths.md`. The skill resolves the output directory from `${CLAUDE_PROJECT_DIR}/.claude/youtube-analyzer.local.md` if present, otherwise prompts the user once via `AskUserQuestion` and persists the answer there for next time.

### Step 4.6: Cleanup

```bash
rm -rf "{scratchpad}/repo-explore/"
```

### Step 4.7: Optional Copy to Invocation Cwd

**Skip when `mode == "chat"`** — no file exists to copy. Set `copyPath = null`.

After the canonical file is written and cleanup is done, ask the user whether to also drop a copy in the cwd captured at skill invocation. Use AskUserQuestion:

```json
{
  "questions": [{
    "question": "Also save a copy to the project you ran this from? Cwd: {invocationCwd}",
    "header": "Copy to repo",
    "options": [
      {"label": "No, just the configured output dir", "description": "File stays at {outputPath} only. No copy created."},
      {"label": "Yes, copy to cwd root", "description": "Drop a copy at {invocationCwd}/{filename}"},
      {"label": "Yes, custom subpath", "description": "I'll specify a subdirectory under {invocationCwd}"}
    ],
    "multiSelect": false
  }]
}
```

**Branch on response:**

- **No** → `copyPath = null`. Done.
- **Yes, copy to cwd root** → `targetDir = invocationCwd`. Proceed to copy.
- **Yes, custom subpath** → ask one follow-up free-text question: *"Subpath under `{invocationCwd}` (e.g., `docs/research`, leave blank for cwd root):"*. Resolve `targetDir = invocationCwd + (subpath || "")`. Proceed to copy.

**Copy execution:**

```bash
mkdir -p "{targetDir}"
cp "{outputPath}" "{targetDir}/{filename}"
```

Set `copyPath = "{targetDir}/{filename}"`.

**Safety guards:**
- If `invocationCwd` is unset (rare — only if cwd capture failed), skip the prompt and set `copyPath = null`.
- If `invocationCwd` equals the configured output directory, skip the prompt — copying onto itself is a no-op.
- If the destination file already exists, append `-2`, `-3`, etc. to the filename rather than overwriting.

---

## Example

**Input:** `"Analyze this video: https://youtube.com/watch?v=xyz123"`

- **Phase 1:** Auto-detect URL → yt-dlp metadata → `youtube_transcript_api` transcript → 12K words.
- **Phase 2:** Detect finance/finance at 87%. User confirms. Selects: Strategy Breakdown + Action Items. Depth: standard. Focus: actionable takeaways.
- **Phase 3:** 12K tokens → single agent. No repo exploration.
- **Phase 4:** 1 FinanceAgent analyzes full transcript. Synthesis writes to `{configured-output-dir}/2026-04-27-dividend-portfolio-strategy.md`. Step 4.7 then asks whether to copy a duplicate into `invocationCwd`.

For `--chat` invocation, Phase 4 returns `renderedMarkdown` inline, skips Steps 4.5 and 4.7, and never writes a file.

---

## Quick Reference

**Scripts** (in `${CLAUDE_PLUGIN_ROOT}/scripts/`): `clean-transcript.ts`, `detect-content-type.ts`, `partition-transcript.ts`, `package-db.ts` · External: `yt-dlp`, `youtube_transcript_api`, `bun`

**Agent types:**
- Workflow agents: `general-purpose` + `sonnet`, given full workflow file content
- Repo explorers: `Explore` (Structure / Dependency / Pattern)
- Synthesizer: `general-purpose` + `sonnet`, given merged results + `references/output-templates.md`

**Workflow files:** `workflows/{tutorial,finance,general,repo-exploration}-workflow.md`

**Sequence:** verify external tools → Phase 1 (Gate 1) → Phase 2 (Gate 2) → Phase 3 (Gate 3) → Phase 4 (Gate 4) → report. No phase may be skipped.
