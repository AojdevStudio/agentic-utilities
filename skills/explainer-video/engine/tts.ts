// narration.json -> audio/<id>.mp3 + audio/<id>.meta.json (one ElevenLabs clip per scene), then timeline.json.
//   bun tts.ts <projectDir>              generate scenes whose text or settings changed, then write the timeline
//   bun tts.ts <projectDir> --force      regenerate every scene
//   bun tts.ts <projectDir> --timeline   only rebuild timeline.json from the clips already in audio/
// The API key and voice ID come from ELEVENLABS_API_KEY and ELEVENLABS_VOICE_ID, or from hush when those are unset.
// They stay in memory. They are never printed, logged, or written.
import { $ } from "bun";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { type } from "arktype";
import {
  LEAD,
  TAIL,
  cueTimes,
  exec,
  formatLength,
  loadAudioMeta,
  loadNarration,
  projectPaths,
  runMain,
  type AudioMeta,
  type ProjectPaths,
  type Scene,
  type TimelineScene,
} from "./contract.ts";

const MODEL = "eleven_multilingual_v2";
const OUTPUT_FORMAT = "mp3_44100_192";
const VOICE_SETTINGS = { stability: 0.5, similarity_boost: 0.8, style: 0.15, use_speaker_boost: true, speed: 1.2 };
const API = "https://api.elevenlabs.io/v1";

const TtsResponse = type({
  audio_base64: "string",
  alignment: { characters: "string[]", character_start_times_seconds: "number[]" },
});
const VoiceResponse = type({ name: "string" });

type Credentials = { key: string; voice: string };

type HushResult = { ok: true; value: string } | { ok: false; error: string };

async function hushGet(name: string): Promise<HushResult> {
  const r = await $`hush get --key ${name}`.quiet().nothrow();
  const value = r.stdout.toString().trim();
  return r.exitCode === 0 && value ? { ok: true, value } : { ok: false, error: r.stderr.toString().trim() || `exit ${r.exitCode}` };
}

// The environment wins. hush runs only for a variable that is unset or blank, and is tried under each name in order.
async function secret(env: string, hushNames: string[]): Promise<string> {
  const fromEnv = process.env[env]?.trim();
  if (fromEnv) return fromEnv;
  if (!Bun.which("hush")) throw new Error(`${env} is not set (and hush, the fallback, is not on PATH)`);
  const errors: string[] = [];
  for (const name of hushNames) {
    const r = await hushGet(name);
    if (r.ok) return r.value;
    errors.push(`${name}: ${r.error}`);
  }
  throw new Error(`${env} is not set and hush has none of ${hushNames.join(", ")} (${errors.join("; ")})`);
}

async function credentials(): Promise<Credentials> {
  const key = await secret("ELEVENLABS_API_KEY", ["ELEVEN_LABS_API_KEY", "Eleven_LABS_API_KEY"]);
  const voice = await secret("ELEVENLABS_VOICE_ID", ["ELEVENLABS_VOICE_ID"]);
  return { key, voice };
}

// API errors can echo the voice ID back, so every message leaving this module passes through here.
const redact = (text: string, c: Credentials) => text.split(c.key).join("<key>").split(c.voice).join("<voice>");

async function api(c: Credentials, path: string, init: RequestInit = {}): Promise<unknown> {
  let res: Response;
  try {
    res = await fetch(`${API}${path}`, { ...init, headers: { "xi-api-key": c.key, "content-type": "application/json" } });
  } catch (e) {
    throw new Error(redact(`ElevenLabs request failed: ${e instanceof Error ? e.message : String(e)}`, c));
  }
  if (!res.ok) throw new Error(redact(`ElevenLabs HTTP ${res.status}: ${await res.text()}`, c));
  return res.json();
}

// The voice is part of the cache key so a voice change regenerates every clip. Only the digest is stored.
const sceneHash = (s: Scene, voice: string) =>
  createHash("sha256")
    .update(JSON.stringify({ text: s.spoken, model: MODEL, format: OUTPUT_FORMAT, settings: VOICE_SETTINGS, voice }))
    .digest("hex");

async function generate(paths: ProjectPaths, s: Scene, c: Credentials, hash: string) {
  const body = JSON.stringify({ text: s.spoken, model_id: MODEL, voice_settings: VOICE_SETTINGS });
  const j = TtsResponse(
    await api(c, `/text-to-speech/${c.voice}/with-timestamps?output_format=${OUTPUT_FORMAT}`, { method: "POST", body }),
  );
  if (j instanceof type.errors) throw new Error(`${s.id}: unexpected ElevenLabs response: ${j.summary}`);
  const t = j.alignment.character_start_times_seconds;
  if (!t.length) throw new Error(`${s.id}: ElevenLabs returned no character timings`);
  await Bun.write(join(paths.audio, `${s.id}.mp3`), Buffer.from(j.audio_base64, "base64"));
  const meta: AudioMeta = { hash, cues: cueTimes(s, t), times: t };
  await Bun.write(join(paths.audio, `${s.id}.meta.json`), JSON.stringify(meta));
}

async function synthesize(paths: ProjectPaths, scenes: Scene[], force: boolean) {
  const c = await credentials();
  const voice = VoiceResponse(await api(c, `/voices/${c.voice}`));
  console.log(`voice: ${voice instanceof type.errors ? "(name unavailable)" : voice.name}`);
  mkdirSync(paths.audio, { recursive: true });
  for (const s of scenes) {
    const hash = sceneHash(s, c.voice);
    const metaPath = join(paths.audio, `${s.id}.meta.json`);
    const cached =
      !force && existsSync(metaPath) && existsSync(join(paths.audio, `${s.id}.mp3`)) && (await loadAudioMeta(metaPath)).hash === hash;
    if (cached) {
      console.log(`${s.id} cached`);
      continue;
    }
    await generate(paths, s, c, hash);
    console.log(`${s.id} generated, ${s.spoken.length} chars`);
  }
}

async function audioDuration(path: string) {
  if (!existsSync(path)) throw new Error(`missing clip ${path} (run tts.ts without --timeline)`);
  const r = await exec(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path]);
  const d = Number(r.stdout.toString().trim());
  if (!Number.isFinite(d) || d <= 0) throw new Error(`ffprobe gave no duration for ${path}`);
  return d;
}

async function writeTimeline(paths: ProjectPaths, scenes: Scene[]) {
  let start = 0;
  const out: TimelineScene[] = [];
  const legacy: string[] = [];
  for (const s of scenes) {
    const audioDur = await audioDuration(join(paths.audio, `${s.id}.mp3`));
    const meta = await loadAudioMeta(join(paths.audio, `${s.id}.meta.json`));
    if (!meta.times) legacy.push(s.id);
    const cues = meta.times ? cueTimes(s, meta.times) : meta.cues;
    const missing = [...s.marks.keys()].filter((m) => !Object.hasOwn(cues, m));
    if (missing.length) throw new Error(`${s.id}: audio meta lacks cues ${missing.join(", ")} (regenerate the clip)`);
    const dur = LEAD + audioDur + TAIL;
    out.push({ id: s.id, title: s.title, start, dur, lead: LEAD, audioDur, cues, closing: s.closing, stills: s.stills });
    start += dur;
  }
  if (legacy.length) console.log(`${legacy.join(", ")}: clips predate per-character timings, so a moved cue keeps its old time there (re-voice with --force once)`);
  await Bun.write(paths.timeline, JSON.stringify({ total: start, scenes: out }, null, 1));
  console.log(`total ${start.toFixed(2)} s (${formatLength(start)}), ${scenes.length} scenes`);
}

runMain(async () => {
  const { values, positionals } = parseArgs({
    args: Bun.argv.slice(2),
    options: { force: { type: "boolean", default: false }, timeline: { type: "boolean", default: false } },
    allowPositionals: true,
  });
  const [dir] = positionals;
  if (!dir || positionals.length > 1) throw new Error("usage: bun tts.ts <projectDir> [--force | --timeline]");
  const paths = projectPaths(dir);
  const scenes = await loadNarration(paths);
  if (!values.timeline) await synthesize(paths, scenes, values.force);
  await writeTimeline(paths, scenes);
});
