# interactive-artifacts

Browser-based interactive artifact MVP for pi.

## What it adds

- `/artifact-explain <topic>`
  - starts a localhost artifact server
  - opens a browser tab
  - asks pi to build a **concept explainer artifact** in that tab
- `/artifact-open [artifact-id]`
  - reopens the latest/current artifact
- `artifact_publish`
  - tool for publishing the full artifact state
- `artifact_get`
  - tool for fetching the current artifact JSON + recent pinned comments

## Workflow

1. Run `/artifact-explain pgbouncer`
2. Pi opens a browser tab on `127.0.0.1`
3. Pi generates the first explainer revision with `artifact_publish`
4. Click a section/glossary card/summary block in the browser
5. Pin feedback in the sidebar
6. The browser sends the feedback back into pi
7. Pi uses `artifact_get` + `artifact_publish` to revise the artifact
8. The browser updates live through SSE

## Notes

- The server binds to `127.0.0.1` only.
- Comment POSTs require a per-artifact token.
- Artifact state is persisted in session history through `artifact_publish` tool result details.
- Pinned comments are persisted as custom session entries.

## Current scope

This MVP is intentionally **Layer 2**:

- external browser tab
- local HTTP callback
- live browser updates
- concept explainer artifacts only

It is **not** yet a full Claude-style generic channels system.
