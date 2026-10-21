---
name: explainer-video
version: 1.0.0
description: "Make a narrated explainer video from a brief. Use when the user asks for an explainer video, wants an answer as a video instead of text, or asks for a 3Blue1Brown-style explanation."
metadata:
  category: media
  lanes: [claude, codex, pi]
  effort: high
---

# explainer-video

A video earns its place when the viewer would skip a written answer. Style is 3Blue1Brown: one idea per scene, shapes and arrows that build on screen as the narrator says the word, and minimal text. Bullet slides fail the brief.

The engine is deterministic. ElevenLabs reads `narration.json` and returns per-character timings. A canvas program draws any frame from `seek(t)`. Headless Chromium plus ffmpeg render it frame by frame. Your work is the script and the scenes. The engine owns timing, rendering, audio, and loudness.

## Where it runs

Render on any machine with bun, ffmpeg, and headless Chromium. A Linux box over ssh works well. Non-interactive ssh skips the shell rc, so on a remote box start every command with a PATH that finds bun, for example:

```bash
export PATH=$HOME/.bun/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH
```

The engine is the `engine/` folder next to this SKILL.md. Below, `<skill>` means this skill's folder, for example `~/.agents/skills/explainer-video`.

### Setup

Run once on the render machine:

```bash
bun install --cwd <skill>/engine
bunx playwright install chromium-headless-shell
export ELEVENLABS_API_KEY=...    # your ElevenLabs API key
export ELEVENLABS_VOICE_ID=...   # the voice that narrates
```

- `ffmpeg` and `ffprobe` must be on PATH.
- Create the API key in ElevenLabs under Developers, API Keys. A restricted key needs Text to Speech access and read access to Voices.
- Pick the voice in ElevenLabs under Voices. To use one you do not have yet, add it from the Voice Library to My Voices first. Then open the voice's menu and copy its voice ID.
- `codex` (the OpenAI Codex CLI) is optional. Only the review in step 6 uses it.

If bun, ffmpeg, Chromium, or either variable is missing, say which one and stop.

A project is a plain folder on the render box, for example `~/explainer-<slug>-<yyyymmdd>`, holding three files you write: `brief.md`, `narration.json`, and `scenes.js`. The engine writes every output into that folder. The worked example is `examples/explainer-video-skill/`. Read all three of its files before you write your own.

## Steps

1. **Brief.** Copy the source facts into `brief.md` as short bullets. Add nothing. The review checks every on-screen claim against this file. Done when each fact the video states has a line in `brief.md`.

2. **Script.** Write `narration.json`: an array of `{ id, title, text, closing?, stills? }`, one entry per scene.
   - Budget about 160 spoken words per minute. A 3 minute target is about 480 words for the whole video. Cut words before you raise the speed.
   - Speak plainly to the viewer as "you". Explain each technical term in one plain line the first time.
   - Spell names the way they sound, because the voice reads letters literally: `P-stack`, `T three`, `A D R four`. On-screen text keeps the real spelling.
   - Put a `{cue}` mark right before the word each visual should land on. Cue names are lowercase, unique within a scene, and are what `scenes.js` animates on.
   - Set `closing: true` on a final summary scene, which hides its scene number. Add `stills: ["cue"]` to capture an extra review still 0.4 s before that cue, at phase changes.
   - Done when every scene has its cues and the word count fits the target.

3. **Voice.** `bun <skill>/engine/tts.ts <project>` reads the key from `ELEVENLABS_API_KEY` and the voice from `ELEVENLABS_VOICE_ID`. For either one that is unset, it tries `hush` (a Bitwarden Secrets Manager CLI) if you have it. Most users just export the two variables. Then it writes `audio/` and `timeline.json` and prints the total length and the voice name. It caches by scene, so a re-run only re-voices changed scenes. `--force` re-voices every scene. Never echo, log, or pass a key or voice ID yourself. Done when the total fits the target. If it does not, cut the script and re-run.

4. **Scenes.** Write `scenes.js`, one `SCENES.<id> = (t, c, dur) => { ... }` per narration scene. Read [references/scenes.md](references/scenes.md) for the drawing kit and the layout rules. Done when every narration id has a scene, and every animation starts from `c("cue")`, never from a raw number of seconds.

5. **Check and stills.** `bun .../engine/render.ts stills <project>` runs `check` first. `check` fails on page errors, missing cues, em dashes or semicolons, and text clipped by the frame. Then it writes `stills/<id>_<n>.png`. Open every still and look at it yourself. Fix text that overlaps, arrows that point at nothing, and labels that are hard to read. Done when `check: PASS` prints and you would ship every still.

6. **Second-model review.** `bun .../engine/review.ts <project>` sends every still plus `brief.md` to the Codex CLI (`gpt-6.1-sol` by default, change it with `--model`) and prints `review<N>.md`. This step is optional. Fix each finding, re-run step 5, then review again, for at most 3 rounds. If `codex` is not installed or fails, say so plainly and do a careful self-review against the same five checks in `engine/review-prompt.md`. Done when the review prints `CLEAN`, or after round 3 with the open findings named.

7. **Render.** `bun .../engine/render.ts video <project> 7`, where 7 is the worker count. It writes `out/final.mp4`: 1920x1080, 30 fps, H.264 with AAC, -14 LUFS. Watch-check it:
   - `ffprobe` shows the duration, plus one video stream and one audio stream.
   - `ffmpeg -i out/final.mp4 -af ebur128 -f null -` shows `I:` near -14 LUFS.
   - Frames pulled with `ffmpeg -ss <t>` at 4 or more points match what the narration says at that moment.

   Done when all three hold.

8. **Deliver.** Share `out/final.mp4` however you share files. If you rendered on a remote box, copy it off with `scp` first.

9. **Report and clean up.** Report:
   - where the video is
   - the duration
   - a table of scenes with start times from `timeline.json`
   - the voice name
   - each review round and what it fixed
   - anything skipped, with the reason

   Then delete the project folder on the render box. It holds the voice cache that makes a revision cheap, so delete it only after the video is delivered and no revision is pending.

## Gotchas

- Text a viewer must read uses `COL.dim` or brighter. `COL.faint` is for decoration and struck-out items only. Codex flags faint labels as unreadable, as it did with three of them in an earlier video.
- `review.ts` exits 1 only when Codex itself fails. Findings exit 0, so read the printed review.
- The example's `scenes.js` is a byte-for-byte reference for the engine. Edit a copy, not the example.
