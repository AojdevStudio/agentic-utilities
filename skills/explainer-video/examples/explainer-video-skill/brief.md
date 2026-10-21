# Explainer video about the explainer-video skill

Audience: developers on X and LinkedIn who use coding agents. The viewer is "you".

- The explainer-video skill makes a narrated explainer video from a brief.
- Style is 3Blue1Brown: one idea per scene, shapes and arrows that build on screen as the narrator says the word, minimal text. Bullet slides fail the brief.
- Uses: explain a pull request, explain a concept, explain a big plan. A video earns its place when you would skip the written answer.
- The agent writes three files: brief.md (the facts), narration.json (the script, one entry per scene), scenes.js (one drawing function per scene).
- The engine is deterministic. The agent's work is the script and the scenes. The engine owns timing, rendering, audio, and loudness.
- In the script, a {cue} mark goes right before the word each visual should land on.
- ElevenLabs reads narration.json aloud and returns per-character timings. Each cue becomes the time that word is spoken.
- Voice is cached per scene. A re-run only re-voices scenes whose text changed.
- A canvas program draws any frame from seek(t), a single time value.
- Headless Chromium draws every frame. ffmpeg joins the frames with the voice.
- Output: out/final.mp4, 1920x1080, 30 fps, H.264 video with AAC audio, loudness -14 LUFS.
- A check runs first. It fails on page errors, missing cues, em dashes or semicolons, and text clipped by the frame. Then it writes stills from every scene.
- Second-model review: Codex looks at every still plus brief.md and lists wrong facts, unreadable labels, clipped or overlapping text, broken arrows. The agent fixes them, for at most 3 rounds, until the review prints CLEAN.
- Script budget: about 160 spoken words per minute.
- This video was made with the skill.
- The skill is open source in the agentic-utilities repo (github.com/AojdevStudio/agentic-utilities).
- Requirements: Bun, ffmpeg, headless Chromium, an ElevenLabs API key and voice ID. Codex is used for the review.
- Cost, measured from session transcripts: one recent video made with the skill (2:46 long) used 4.44M Claude tokens: 4,198,398 cache reads, 222,634 cache writes, 23,707 output tokens. Cache reads were about 95 percent.
- Priced at Claude Opus API list rates (input $4, output $20, cache read $0.20, 1-hour cache write $8 per million tokens) that video cost $3.10. On a subscription plan the real cost is plan usage, not dollars.
- That video's voice was 2,462 ElevenLabs characters. Its Codex review used 80,185 tokens, one round.
- The render itself makes no model calls, so it costs no tokens.
