# Phase 1 — Source Selection (Detailed Mechanics)

Loaded by the orchestrator when starting Phase 1.

## Step 1.1: Resolve YouTube URL

If the user already pasted a YouTube URL in their message, skip the prompt and use it. Otherwise, ask:

```json
{
  "questions": [{
    "question": "Paste the YouTube URL you want to analyze.",
    "header": "YouTube URL",
    "options": [
      {"label": "Provide URL", "description": "Paste a public YouTube video URL"}
    ],
    "multiSelect": false
  }]
}
```

(If your end user inputs a non-URL, free-text response will be captured via the question's "Other" option.)

> The personal version of this skill also supported browsing a local transcript library by topic / channel / keyword. That option is dropped from the public plugin because it depends on a user-specific directory layout. If you maintain a local transcript repo, fork the plugin and re-add the browse flow.

---

## Step 1.2: Extract Metadata

```bash
yt-dlp --dump-json --skip-download "URL"
```

Extract: `title, description, channel, tags, duration, category_id, upload_date, view_count, video_id`.

---

## Step 1.3: Fetch Transcript (4-tier fallback chain)

**Tier 1: youtube_transcript_api (preferred)**

- Extract video ID from URL
- Run: `youtube_transcript_api {VIDEO_ID} --format json`
- Parse JSON: array of `{ "text", "start", "duration" }`
- Reconstruct clean transcript, preserve timestamps
- Set `transcriptQuality: "HIGH"`

**Tier 2: yt-dlp (fallback)**

- Run: `yt-dlp --skip-download --write-auto-sub --sub-lang en --sub-format vtt "URL"`
- If auto-sub fails, try `--write-sub` for manual subs
- Process .vtt → clean text (strip timestamps, formatting, duplicates) using `${CLAUDE_PLUGIN_ROOT}/scripts/clean-transcript.ts`
- Set `transcriptQuality: "MEDIUM"`

**Tier 3: metadata-only (last resort)**

- If Step 1.2 succeeded: use title + description + tags as analysis input
- If Step 1.2 also failed: `yt-dlp --dump-json --skip-download "URL" 2>/dev/null || echo "METADATA_FAILED"`
- Set `transcriptQuality: "NONE"`
- Warn user: "No transcript available. Analysis will be limited to video metadata."

**Tier 4: graceful exit (all tiers failed)**

- Set `transcriptQuality: "UNAVAILABLE"`
- Report: "Transcript and metadata both unavailable. This can happen with private, deleted, age-restricted, or region-locked videos. Try a different video or provide the transcript manually."
- **STOP processing.** Skill exits here — do not proceed to Phase 2.

---

## Step 1.4: Save Transcript to Scratchpad

Save raw and cleaned transcripts to a scratchpad directory. Default scratchpad:

```
${CLAUDE_PROJECT_DIR}/.youtube-analyzer-scratch/
```

If `CLAUDE_PROJECT_DIR` is unset, fall back to `./.youtube-analyzer-scratch/` relative to the current working directory.

Save raw transcript to: `{scratchpad}/transcript-{video_id}.txt`

---

## Step 1.5: Clean Transcript

Run the cleanup script to strip VTT artifacts and produce a normalized text file:

```bash
bun run ${CLAUDE_PLUGIN_ROOT}/scripts/clean-transcript.ts \
  --input "{rawTranscriptPath}" \
  --output-dir "{scratchpad}"
```

The script:

1. Strips VTT header lines (`Kind: captions`, `Language: en`)
2. Strips inline timestamp lines (`<c>` tags, `<00:00:` patterns)
3. Deduplicates consecutive identical lines
4. Collapses 3+ blank lines to single blank
5. Preserves YAML frontmatter (parses for metadata)
6. Writes clean text to scratchpad
7. Returns: `{ cleanPath, wordCount, lineCount, metadata }`

**Manual fallback** if the script is unavailable: apply rules 1–5 in order, write to `{scratchpad}/clean-transcript.txt`, count words and lines.

---

## Required External Tools

The end user must have these installed:

- **`yt-dlp`** — `pip install yt-dlp` or `brew install yt-dlp`
- **`youtube_transcript_api`** — `pip install youtube-transcript-api`
- **`bun`** — `curl -fsSL https://bun.sh/install | bash` (used to run the TypeScript scripts in `${CLAUDE_PLUGIN_ROOT}/scripts/`)

If any are missing, surface a clear install message before proceeding.
