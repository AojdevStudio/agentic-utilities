# Phase 3 — Scaling + GitHub Repo Exploration

Loaded by the orchestrator at the start of Phase 3.

## Token Estimation

```
Estimate transcript token count:
- wordCount * 1.3 = approximate token count
- OR: lineCount * 15 = approximate token count (cleaned transcripts average ~15 tokens/line)
```

## Scaling Decision

| Estimated Tokens | Strategy | Details |
|------------------|----------|---------|
| Under 30K | Single agent | One workflow agent gets the full transcript |
| 30K – 100K | 2–3 agents | Split into chunks respecting sentence boundaries |
| Over 100K | 4+ agents | Split into ~25K token chunks |

## Partition Execution

```bash
bun run ${CLAUDE_PLUGIN_ROOT}/scripts/partition-transcript.ts \
  --input "{cleanTranscriptPath}" \
  --max-lines-per-agent 5000
```

**Manual fallback** if tool unavailable:
- Count lines in clean transcript
- Divide into chunks of ~1700 lines each (~25K tokens)
- Write each chunk to scratchpad: `{scratchpad}/chunk-{N}.txt`

---

## GitHub Repo Exploration (Tutorial / Course Only)

**Trigger:** `repoUrl` is not null AND format is `tutorial` or `course`. Run in parallel with transcript partitioning.

### Step 1 — Clone repo

```bash
git clone --depth 1 "{repoUrl}" "{scratchpad}/repo-explore/"
```

**Edge cases (handle before cloning):**
- **Private repos:** Ask user for token or skip exploration
- **Large repos (>10K files):** Warn user, offer quick scan or skip
- **Monorepos:** Ask which workspace to analyze
- **Clone failure:** Skip gracefully, set `repoExploreResults = null`, note reason in output

### Step 2 — Spawn 3 Explore agents in PARALLEL

Each agent uses `subagent_type: Explore`. See `workflows/repo-exploration.md` for the detailed agent prompts.

1. **StructureExplorer** — file organization, architecture, entry points
   → Mermaid `graph TD` of project structure and component relationships
2. **DependencyExplorer** — `package.json` deps with exact versions, configs
   → Mermaid `graph LR` of dependency tree (core vs dev vs optional)
3. **PatternExplorer** — data flow, state management, auth, API patterns
   → Mermaid `flowchart` and `sequenceDiagram` of key patterns

### Step 3 — Collect results

Merge all 3 agent outputs into `repoExploreResults`:

```json
{
  "structure": "```mermaid\ngraph TD\n  ...\n```",
  "dependencies": "```mermaid\ngraph LR\n  ...\n```",
  "patterns": "```mermaid\nsequenceDiagram\n  ...\n```",
  "summary": "Brief text summary of findings"
}
```

### Step 4 — Cleanup

**IMPORTANT:** Don't clean up yet. Cleanup happens AFTER synthesis in Phase 4 Step 4.6, in case agents need to re-read files. The Phase 4 cleanup command:

```bash
rm -rf "{scratchpad}/repo-explore/"
```
