# General Workflow

Default analysis workflow for YouTube content that doesn't match specialized formats (tutorial, finance, sermon). Handles interviews, lectures, educational content, and everything else.

## Input

This workflow receives:
- `transcriptChunk` — file path to transcript text (full or chunk)
- `config.focusArea` — one of: "key-insights", "summary-only", "deep-analysis", "quotes-wisdom"
- `config.depth` — one of: "quick" (~500 words), "standard" (~1500 words), "deep" (~3000+ words)
- `metadata` — video title, channel, duration, upload_date
- `chunkInfo` — { chunk: N, startLine: X, endLine: Y } if partitioned, null if full

## Analysis Protocol

1. Read the transcript chunk from the provided file path
2. Identify main themes, arguments, and narrative structure
3. Extract key insights with supporting evidence
4. Note any timestamps referenced in the transcript
5. Identify speakers if multiple (interviews/podcasts)

## Output Modes

### Key Insights Mode (config.focusArea = "key-insights")
Focus on extractable wisdom:
- Top 5-10 key insights, each with supporting quote
- Pattern recognition across topics
- Contrarian or surprising viewpoints
- Actionable takeaways

### Summary Only Mode (config.focusArea = "summary-only")
Concise executive summary:
- 2-3 paragraph overview
- Bullet list of main points
- Who should watch this and why

### Deep Analysis Mode (config.focusArea = "deep-analysis")
Comprehensive breakdown:
- Section-by-section analysis with timestamps
- Argument mapping (claims → evidence → conclusions)
- Critical assessment of claims
- Connections to broader themes
- Gaps or unanswered questions

### Quotes + Wisdom Mode (config.focusArea = "quotes-wisdom")
Focus on memorable content:
- Notable quotes with context and timestamps
- Wisdom extracts (timeless principles)
- Frameworks or mental models mentioned
- Metaphors and analogies used

## Output Structure

Return structured markdown (adapt sections based on depth):

```markdown
## Executive Summary
[2-3 paragraphs]

## Key Topics
1. [Topic] — [brief description]
2. [Topic] — [brief description]

## Detailed Analysis
### [Section/Topic]
[Analysis with quotes and timestamps]

## Notable Quotes
> "Quote text" — [Speaker, timestamp]

## Practical Applications
- [Actionable takeaway]

## Quality Assessment
- Transcript quality: [HIGH/MEDIUM/LOW]
- Content density: [HIGH/MEDIUM/LOW]
- Production value: [description]
```

## Chunk-Specific Instructions

If this is a partitioned chunk (chunkInfo is not null):
- Analyze ONLY the content in your assigned chunk
- Note if topics span chunk boundaries (incomplete thoughts at start/end)
- Label all findings with approximate timestamps if available
- Do NOT attempt to read other chunks or the full transcript
- The synthesis agent will merge your analysis with other chunks
