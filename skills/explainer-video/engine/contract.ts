// The project contract: narration.json schema, the derived timeline, and the helpers every
// engine script shares. A project is a folder holding narration.json, scenes.js, and brief.md.
// All outputs (audio/, timeline.json, stills/, frames/, out/) are written into that folder.
import { type } from "arktype";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

export const FPS = 30;
export const WIDTH = 1920;
export const HEIGHT = 1080;
// Silence before and after each narration clip inside its scene.
export const LEAD = 0.35;
export const TAIL = 0.65;
// A still for a `stills` cue lands this long before the cue, so the frame shows what was built so far.
export const STILL_BEFORE_CUE = 0.4;

const SceneSpec = type({
  "+": "reject",
  id: /^[a-z0-9_]+$/,
  title: "string > 0",
  text: "string",
  "closing?": "boolean",
  "stills?": "string[]",
});
const NarrationSpec = SceneSpec.array().atLeastLength(1);
type SceneSpec = typeof SceneSpec.infer;

const Cues = type("Record<string, number>");
export type Cues = typeof Cues.infer;

const TimelineSceneSpec = type({
  id: "string",
  title: "string",
  start: "number",
  dur: "number",
  lead: "number",
  audioDur: "number",
  cues: Cues,
  closing: "boolean",
  stills: "string[]",
});
const TimelineSpec = type({ total: "number", scenes: TimelineSceneSpec.array() });
export type TimelineScene = typeof TimelineSceneSpec.infer;
export type Timeline = typeof TimelineSpec.infer;

// audio/<id>.meta.json: the cache key of the clip, its cue times (seconds into the clip), and the start time of
// every spoken character. Cues are recomputed from `times` when the timeline is written, so moving or renaming a
// cue needs no new audio. Clips cached before `times` existed carry only `cues`.
export const AudioMeta = type({ hash: "string", cues: Cues, "times?": "number[]" });
export type AudioMeta = typeof AudioMeta.infer;

/** Each `{cue}` mark's time: the start time of the character it sits before. */
export const cueTimes = (s: Scene, times: readonly number[]): Record<string, number> =>
  Object.fromEntries([...s.marks].map(([name, i]) => [name, times[Math.min(i, times.length - 1)]!]));

/** A length in seconds as m:ss, rounded to the nearest second first so 119.6 s reads 2:00, never 1:60. */
export function formatLength(seconds: number) {
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

/** A validated scene: the spoken text with `{cue}` marks removed, and each mark's character offset. */
export type Scene = {
  id: string;
  title: string;
  spoken: string;
  marks: Map<string, number>;
  closing: boolean;
  stills: string[];
};

export class ContractError extends Error {
  constructor(file: string, problems: string[]) {
    super(`${file} is invalid:\n${problems.map((p) => `  - ${p}`).join("\n")}`);
  }
}

const CUE_SPLIT = /(\{[a-z0-9_]+\})/;
const CUE_MARK = /^\{([a-z0-9_]+)\}$/;

// Same walk as the raw build: a mark's offset is the length of spoken text before it.
function parseText(id: string, text: string, problems: string[]) {
  const marks = new Map<string, number>();
  let spoken = "";
  for (const part of text.split(CUE_SPLIT)) {
    const name = part.match(CUE_MARK)?.[1];
    if (name !== undefined) {
      if (marks.has(name)) problems.push(`${id}.text: duplicate cue {${name}}`);
      marks.set(name, spoken.length);
    } else {
      const bad = part.match(/[{}]/);
      if (bad?.index !== undefined) {
        const near = part.slice(Math.max(0, bad.index - 12), bad.index + 13);
        problems.push(`${id}.text: bad cue syntax near "${near}" (cues are {name} with a-z, 0-9, _)`);
      }
      spoken += part;
    }
  }
  if (spoken.trim() === "") problems.push(`${id}.text: empty after removing {cue} marks`);
  return { spoken, marks };
}

export function validateNarration(data: unknown, file = "narration.json"): Scene[] {
  const parsed = NarrationSpec(data);
  if (parsed instanceof type.errors) {
    const label = (i: PropertyKey | undefined) => {
      const row = Array.isArray(data) && typeof i === "number" ? (data[i] as { id?: unknown } | undefined) : undefined;
      return typeof row?.id === "string" ? row.id : `scene #${String(i)}`;
    };
    throw new ContractError(
      file,
      parsed.map((e) => {
        const [i, ...rest] = e.path;
        return e.path.length ? `${label(i)}${rest.length ? `.${rest.join(".")}` : ""}: ${e.problem}` : e.problem;
      }),
    );
  }
  const problems: string[] = [];
  const seen = new Set<string>();
  const scenes = parsed.map((s: SceneSpec): Scene => {
    if (seen.has(s.id)) problems.push(`${s.id}.id: duplicate scene id`);
    seen.add(s.id);
    const { spoken, marks } = parseText(s.id, s.text, problems);
    const stills = s.stills ?? [];
    stills.forEach((cue, i) => {
      if (!marks.has(cue)) problems.push(`${s.id}.stills[${i}]: "${cue}" is not a cue in ${s.id}`);
    });
    return { id: s.id, title: s.title, spoken, marks, closing: s.closing ?? false, stills };
  });
  if (problems.length) throw new ContractError(file, problems);
  return scenes;
}

export function projectPaths(dir: string) {
  const root = resolve(dir);
  return {
    root,
    narration: join(root, "narration.json"),
    scenes: join(root, "scenes.js"),
    brief: join(root, "brief.md"),
    timeline: join(root, "timeline.json"),
    audio: join(root, "audio"),
    stills: join(root, "stills"),
    frames: join(root, "frames"),
    out: join(root, "out"),
  };
}
export type ProjectPaths = ReturnType<typeof projectPaths>;

async function readJson(path: string, what: string): Promise<unknown> {
  if (!existsSync(path)) throw new Error(`missing ${what}: ${path}`);
  try {
    return await Bun.file(path).json();
  } catch (e) {
    throw new Error(`${path} is not valid JSON: ${errorText(e)}`);
  }
}

export async function loadNarration(paths: ProjectPaths): Promise<Scene[]> {
  return validateNarration(await readJson(paths.narration, "narration.json"), paths.narration);
}

export async function loadAudioMeta(path: string): Promise<AudioMeta> {
  const meta = AudioMeta(await readJson(path, "audio meta"));
  if (meta instanceof type.errors) throw new Error(`${path}: ${meta.summary}`);
  return meta;
}

/** Loads timeline.json and proves it was built from the current narration.json. */
export async function loadTimeline(paths: ProjectPaths, narration: Scene[]): Promise<Timeline> {
  const tl = TimelineSpec(await readJson(paths.timeline, "timeline.json (run tts.ts first)"));
  if (tl instanceof type.errors) throw new Error(`${paths.timeline}: ${tl.summary}`);
  const stale: string[] = [];
  if (tl.scenes.length !== narration.length) stale.push(`${tl.scenes.length} scenes, narration has ${narration.length}`);
  narration.forEach((n, i) => {
    const s = tl.scenes[i];
    if (!s || s.id !== n.id) return stale.push(`scene ${i + 1} is ${s?.id ?? "missing"}, narration has ${n.id}`);
    if (s.title !== n.title) stale.push(`${n.id}.title differs`);
    if (s.closing !== n.closing) stale.push(`${n.id}.closing differs`);
    if (s.stills.join() !== n.stills.join()) stale.push(`${n.id}.stills differs`);
    const cues = Object.keys(s.cues);
    if (cues.length !== n.marks.size || cues.some((c) => !n.marks.has(c))) stale.push(`${n.id} cues differ`);
  });
  if (stale.length) throw new Error(`timeline.json does not match narration.json (run tts.ts):\n  - ${stale.join("\n  - ")}`);
  return tl;
}

/**
 * Still times for one scene, in time order: before each `stills` cue, mid-scene, and the end state.
 * A pre-cue still never lands before `start + lead`, where the fade-in ends, so it stays in its own scene.
 */
export function stillTimes(s: TimelineScene): number[] {
  return [
    ...s.stills.map((cue) => Math.max(s.start + s.lead, s.start + s.lead + s.cues[cue]! - STILL_BEFORE_CUE)),
    s.start + s.dur * 0.5,
    s.start + s.dur - 0.5,
  ].sort((a, b) => a - b);
}
export const stillName = (id: string, n: number) => `${id}_${n}.png`;

export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Runs an external command. A non-zero exit throws with the command's stderr. */
export async function exec(cmd: [string, ...string[]], opts: { timeoutMs?: number } = {}) {
  if (!Bun.which(cmd[0])) throw new Error(`${cmd[0]} is not on PATH`);
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe", timeout: opts.timeoutMs });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).arrayBuffer(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (proc.signalCode) throw new Error(`${cmd[0]} was killed (${proc.signalCode})${opts.timeoutMs ? ` after ${opts.timeoutMs / 1000}s` : ""}: ${stderr.trim()}`);
  if (code !== 0) throw new Error(`${cmd[0]} exited ${code}: ${stderr.trim() || "(no stderr)"}`);
  return { stdout: Buffer.from(stdout), stderr };
}

/** Entry point wrapper: any thrown error prints one plain line and exits 1. */
export function runMain(main: () => Promise<void>) {
  main().catch((e: unknown) => {
    console.error(`error: ${errorText(e)}`);
    process.exit(1);
  });
}
