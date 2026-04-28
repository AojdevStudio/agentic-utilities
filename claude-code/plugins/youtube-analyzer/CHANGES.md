# Changes from the personal (PAI) version

This plugin is a port of the YouTubeAnalyzer skill from the author's personal PAI installation. The following changes were made for public release. They are documented so the original author can reconcile divergence between the two versions.

## Removed

| What | Why |
|------|-----|
| **Transcript repo browse** (Option A in `source-selection.md`) | Depended on the author's local `desktop-commander/repos/playlist-transcripts/` directory layout. Public version is YouTube-URL-only; users wanting a local library can fork. |
| **Sermon handoff** | Forwarded to the author's separate `SermonAnalyzer` skill. Public version treats `sermon` content under `general-workflow.md`. If a sermon plugin ships separately, this can be reintroduced as a cross-plugin handoff. |
| **`SermonWorkflow.md`** | Same reason as above. |
| **Voice notification** | Hard-coded to `http://localhost:8888/notify` (PAI's local voice server) and an ElevenLabs voice ID. PAI-specific, not portable. |
| **User customization dir** (`${PAI_DIR}/skills/CORE/USER/SKILLCUSTOMIZATIONS/YouTubeAnalyzer/`) | PAI-only directory. Replaced with the `.claude/youtube-analyzer.local.md` plugin-settings pattern. |
| **PAI-specific agent types and terminology** | "Obi", "ISC", "PAI's 7-phase loop", references to the `Algorithm` agent. Generalized to "the orchestrator" and standard Claude Code agent types. |
| **Hard-coded research vault path** (personal absolute notes path) | Replaced with a per-user output directory resolved on first run via `AskUserQuestion`, persisted to `.claude/youtube-analyzer.local.md`. |
| **Hard-coded sermon vault path** (`acp-church-media/sermons/`) | Removed alongside sermon handoff. |

## Renamed

PascalCase → kebab-case to match Claude Code plugin conventions.

| Original | New |
|----------|-----|
| `Tools/CleanTranscript.ts` | `scripts/clean-transcript.ts` |
| `Tools/DetectContentType.ts` | `scripts/detect-content-type.ts` |
| `Tools/PartitionTranscript.ts` | `scripts/partition-transcript.ts` |
| `Tools/PackageDb.ts` | `scripts/package-db.ts` |
| `Workflows/TutorialWorkflow.md` | `workflows/tutorial-workflow.md` |
| `Workflows/FinanceWorkflow.md` | `workflows/finance-workflow.md` |
| `Workflows/GeneralWorkflow.md` | `workflows/general-workflow.md` |
| `Workflows/RepoExploration.md` | `workflows/repo-exploration.md` |
| `ContentTypes.md` | `content-types.md` |
| `PackageDatabaseSchema.md` | `package-database-schema.md` |

## Path replacements

| Original | New |
|----------|-----|
| `${PAI_DIR}/skills/YouTubeAnalyzer/Tools/<file>.ts` | `${CLAUDE_PLUGIN_ROOT}/scripts/<file>.ts` |
| Hard-coded vault path | Resolved from `.claude/youtube-analyzer.local.md` (prompted on first run) |
| `~/Projects/desktop-commander/...` (transcript repo) | (removed — feature dropped) |

## Unchanged behaviors

- 4-phase blocking-gate orchestration model.
- Multi-agent scaling thresholds (≤30K / 30–100K / >100K tokens).
- `subagent_type: "general-purpose"` + `model: "sonnet"` for workflow agents.
- `subagent_type: "Explore"` for the 3 parallel repo explorers (StructureExplorer, DependencyExplorer, PatternExplorer).
- Content-type detection logic and keyword scoring (`content-types.md`).
- Package database schema (`~/.config/youtube-analyzer/package-db.json` was already portable).
- Invocation flags (`--chat`, `--document`).
- `AskUserQuestion`-driven Phase 2 interactive config.
- Optional copy-to-cwd at Step 4.7.

## Audit checklist for future ports

When porting other personal skills into this plugin marketplace, check for:

- Personal absolute paths
- `${PAI_DIR}` or `~/.claude/...` references that won't resolve for end users
- `localhost:8888` (PAI voice server) or other hard-coded local services
- ElevenLabs voice IDs or other private API IDs
- References to other Ossie-only skills (e.g., SermonAnalyzer, OpenClawOps, Telos)
- Hard-coded vault/output paths
- Custom agent types that don't exist in stock Claude Code
