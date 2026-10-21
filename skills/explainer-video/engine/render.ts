// Renders a project's scene program frame by frame in headless Chromium.
//   bun render.ts check <projectDir>             seek the whole timeline, fail on page errors, missing cues, bad text
//   bun render.ts stills <projectDir>            check, then stills/<id>_<n>.png at the moments stillTimes() picks
//   bun render.ts video <projectDir> [workers]   out/final.mp4 with narration at -14 LUFS
import { chromium, type Browser, type Page } from "playwright-core";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { type } from "arktype";
import {
  FPS,
  HEIGHT,
  WIDTH,
  exec,
  loadNarration,
  loadTimeline,
  projectPaths,
  runMain,
  stillName,
  stillTimes,
  type ProjectPaths,
  type Timeline,
} from "./contract.ts";

const ENGINE = import.meta.dir;
const CHECK_STEP = 0.25;

// ---------- font ----------
const FONT_URL = "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip";
const FONT_IN_ZIP = "web/InterVariable.woff2";
// sha256 of web/InterVariable.woff2 from Inter 4.1, the exact file the first video shipped with
const FONT_SHA256 = "693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3";
const CACHE = join(homedir(), ".cache", "explainer-video");
const FONT_PATH = join(CACHE, "InterVariable-4.1.woff2");

const sha256 = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");

async function ensureFont() {
  if (existsSync(FONT_PATH)) {
    const got = sha256(await Bun.file(FONT_PATH).bytes());
    if (got !== FONT_SHA256) throw new Error(`${FONT_PATH} has sha256 ${got}, expected ${FONT_SHA256}. Delete it to re-download.`);
    return;
  }
  console.log(`font: downloading Inter 4.1 into ${CACHE}`);
  mkdirSync(CACHE, { recursive: true });
  const res = await fetch(FONT_URL);
  if (!res.ok) throw new Error(`font download failed: HTTP ${res.status} from ${FONT_URL}`);
  const zip = join(CACHE, "Inter-4.1.zip");
  await Bun.write(zip, res);
  try {
    const { stdout } = await exec(["unzip", "-p", zip, FONT_IN_ZIP]);
    const got = sha256(stdout);
    if (got !== FONT_SHA256) throw new Error(`downloaded ${FONT_IN_ZIP} has sha256 ${got}, expected ${FONT_SHA256}`);
    await Bun.write(FONT_PATH, stdout);
  } finally {
    rmSync(zip, { force: true });
  }
}

// ---------- chromium ----------
const INSTALL = "bunx playwright install chromium-headless-shell";

function findChrome() {
  const platform =
    process.platform === "linux" && process.arch === "x64" ? "linux64"
    : process.platform === "darwin" && process.arch === "arm64" ? "mac-arm64"
    : null;
  if (!platform) throw new Error(`no chrome-headless-shell build for ${process.platform}-${process.arch}`);
  // Where `playwright install` puts browsers, by Playwright's own rules: PLAYWRIGHT_BROWSERS_PATH
  // (relative to INIT_CWD), else ~/Library/Caches on macOS, else XDG_CACHE_HOME or ~/.cache.
  const custom = process.env.PLAYWRIGHT_BROWSERS_PATH;
  if (custom === "0")
    throw new Error(`PLAYWRIGHT_BROWSERS_PATH=0 keeps browsers inside the package that installed them, where this engine cannot see them. Unset it or point it at a directory, then run: ${INSTALL}`);
  const root =
    custom ? resolve(process.env.INIT_CWD ?? process.cwd(), custom)
    : process.platform === "darwin" ? join(homedir(), "Library", "Caches", "ms-playwright")
    : join(process.env.XDG_CACHE_HOME || join(homedir(), ".cache"), "ms-playwright");
  const builds = existsSync(root)
    ? [...new Bun.Glob(`chromium_headless_shell-*/chrome-headless-shell-${platform}/chrome-headless-shell`).scanSync(root)]
        .map((rel) => ({ rev: Number(rel.match(/chromium_headless_shell-(\d+)/)?.[1]), exe: join(root, rel) }))
        .filter((b) => Number.isFinite(b.rev) && existsSync(join(dirname(dirname(b.exe)), "INSTALLATION_COMPLETE")))
        .sort((a, b) => b.rev - a.rev)
    : [];
  const newest = builds[0];
  if (!newest) throw new Error(`chrome-headless-shell is not installed under ${root}. Install it with: ${INSTALL}`);
  return newest;
}

// ---------- page ----------
type TextRecord = { s: string; x0: number; y0: number; x1: number; y1: number; alpha: number };
type EngineWindow = Window & {
  seek: (t: number) => void;
  READY?: boolean;
  BOOT_ERROR?: string;
  TEXT_LOG?: TextRecord[];
};

function serve(paths: ProjectPaths) {
  const files: Record<string, string> = {
    "/": join(ENGINE, "index.html"),
    "/lib.js": join(ENGINE, "lib.js"),
    "/Inter.woff2": FONT_PATH,
    "/scenes.js": paths.scenes,
    "/timeline.json": paths.timeline,
  };
  return Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(req) {
      const path = new URL(req.url).pathname;
      if (path === "/favicon.ico") return new Response(null, { status: 204 });
      const file = files[path];
      return file && existsSync(file) ? new Response(Bun.file(file)) : new Response("not found", { status: 404 });
    },
  });
}

async function openPage(browser: Browser, url: string): Promise<{ page: Page; errors: string[] }> {
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("requestfailed", (r) => errors.push(`request failed: ${r.url()}`));
  page.on("response", (r) => r.status() >= 400 && errors.push(`HTTP ${r.status()} for ${new URL(r.url()).pathname}`));
  await page.goto(url);
  const ok = await page
    .waitForFunction(() => {
      const w = window as unknown as EngineWindow;
      return w.READY === true || typeof w.BOOT_ERROR === "string";
    }, null, { timeout: 20000 })
    .then(() => true, () => false);
  const bootError = ok ? await page.evaluate(() => (window as unknown as EngineWindow).BOOT_ERROR) : "page never became ready";
  if (bootError || errors.length) throw new Error(`page failed to load: ${[bootError, ...errors].filter(Boolean).join(" | ")}`);
  return { page, errors };
}

const frame = (page: Page, T: number) =>
  page.evaluate((T) => {
    (window as unknown as EngineWindow).seek(T);
    return (document.getElementById("c") as HTMLCanvasElement).toDataURL("image/png").split(",")[1]!;
  }, T);

const sceneAt = (tl: Timeline, T: number) =>
  tl.scenes.find((s) => T >= s.start && T < s.start + s.dur) ?? tl.scenes[tl.scenes.length - 1]!;

// ---------- check ----------
type Sample = { T: number; id: string; texts: Map<string, TextRecord> };

// One string drawn n times in a frame gets keys "s#0", "s#1", ... so it can be followed across samples.
function keyed(log: TextRecord[]) {
  const seen = new Map<string, number>();
  return new Map(log.map((t) => {
    const n = seen.get(t.s) ?? 0;
    seen.set(t.s, n + 1);
    return [`${t.s}#${n}`, t] as const;
  }));
}

const moved = (a: TextRecord, b: TextRecord | undefined) => b !== undefined && Math.abs(a.x0 - b.x0) + Math.abs(a.y0 - b.y0) > 0.5;

async function check(page: Page, errors: string[], tl: Timeline) {
  const seekErrors = new Map<string, number>();
  const punctuation = new Map<string, string>();
  const outside = new Map<string, string>();
  const strings = new Set<string>();
  const log: Sample[] = [];
  const samples = Math.floor(tl.total / CHECK_STEP) + 1;
  for (let i = 0; i < samples; i++) {
    const T = Math.min(i * CHECK_STEP, tl.total - 1 / FPS);
    const r = await page.evaluate((T) => {
      const w = window as unknown as EngineWindow;
      const log: TextRecord[] = [];
      w.TEXT_LOG = log;
      try {
        w.seek(T);
        return { log, error: null };
      } catch (e) {
        return { log, error: e instanceof Error ? e.message : String(e) };
      } finally {
        w.TEXT_LOG = undefined;
      }
    }, T);
    const id = sceneAt(tl, T).id;
    if (r.error && !seekErrors.has(r.error)) seekErrors.set(r.error, T);
    for (const t of r.log) {
      strings.add(t.s);
      if (/[—;]/.test(t.s)) punctuation.set(t.s, id);
    }
    log.push({ T, id, texts: keyed(r.log) });
  }

  // Clipped text is visible text that crosses the frame edge. Text with no pixel on screen, or
  // with zero alpha, cannot look clipped. Text that crosses the edge while it moves (compared with
  // the samples either side) is an entrance or exit animation, so it is counted but not failed.
  let inMotion = 0;
  log.forEach(({ T, id, texts }, i) => {
    const near = [log[i - 1], log[i + 1]].filter((s) => s?.id === id);
    for (const [k, t] of texts) {
      const onScreen = t.x1 > 0 && t.x0 < WIDTH && t.y1 > 0 && t.y0 < HEIGHT;
      const crosses = t.x0 < 0 || t.y0 < 0 || t.x1 > WIDTH || t.y1 > HEIGHT;
      if (!onScreen || !crosses || t.alpha <= 0.01) continue;
      if (near.some((s) => moved(t, s?.texts.get(k)))) {
        inMotion++;
        continue;
      }
      const key = `${id} "${t.s}"`;
      const b = [t.x0, t.y0, t.x1, t.y1].map((v) => Math.round(v)).join(",");
      if (!outside.has(key)) outside.set(key, `${key} at ${T.toFixed(2)}s, bounds ${b}`);
    }
  });

  const problems = [
    ...errors.map((e) => `page error: ${e}`),
    ...[...seekErrors].map(([e, T]) => `seek error at ${T.toFixed(2)}s: ${e}`),
    ...[...punctuation].map(([s, id]) => `${id}: em dash or semicolon in on-screen text "${s}"`),
    ...[...outside.values()].map((o) => `clipped by the ${WIDTH}x${HEIGHT} frame: ${o}`),
  ];
  console.log(`check: ${samples} samples every ${CHECK_STEP}s over ${tl.total.toFixed(2)}s, ${tl.scenes.length} scenes`);
  console.log(`  distinct on-screen strings: ${strings.size}`);
  console.log(`  page errors: ${errors.length}, seek errors: ${seekErrors.size}`);
  console.log(`  em dash or semicolon: ${punctuation.size}, clipped by the frame: ${outside.size}`);
  console.log(`  crossing the frame edge while moving (allowed): ${inMotion}`);
  if (problems.length) throw new Error(`check failed:\n  - ${problems.join("\n  - ")}`);
  console.log("check: PASS");
}

// ---------- stills ----------
async function stills(page: Page, paths: ProjectPaths, tl: Timeline) {
  mkdirSync(paths.stills, { recursive: true });
  for (const f of readdirSync(paths.stills)) if (f.endsWith(".png")) rmSync(join(paths.stills, f));
  for (const s of tl.scenes) {
    const times = stillTimes(s);
    for (const [n, T] of times.entries()) {
      await Bun.write(join(paths.stills, stillName(s.id, n)), Buffer.from(await frame(page, T), "base64"));
    }
    console.log("stills", s.id, times.map((x) => x.toFixed(1)).join(" "));
  }
}

// ---------- video ----------
const Loudnorm = type({ input_i: "string", input_tp: "string", input_lra: "string", input_thresh: "string", target_offset: "string" });

async function video(browser: Browser, url: string, paths: ProjectPaths, tl: Timeline, workers: number) {
  mkdirSync(paths.frames, { recursive: true });
  mkdirSync(paths.out, { recursive: true });
  const total = Math.ceil(tl.total * FPS);
  const per = Math.ceil(total / workers);
  const t0 = performance.now();
  await Promise.all(
    Array.from({ length: workers }, async (_, w) => {
      const from = w * per, to = Math.min(total, from + per);
      const { page } = await openPage(browser, url);
      const ff = Bun.spawn(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "image2pipe", "-c:v", "png", "-framerate", String(FPS), "-i", "-",
          "-c:v", "libx264", "-preset", "medium", "-crf", "16", "-pix_fmt", "yuv420p", "-r", String(FPS), join(paths.frames, `seg_${w}.mp4`)],
        { stdin: "pipe", stdout: "ignore", stderr: "pipe" },
      );
      const stderr = new Response(ff.stderr).text();
      for (let f = from; f < to; f++) {
        ff.stdin.write(Buffer.from(await frame(page, f / FPS), "base64"));
        await ff.stdin.flush();
        if (w === 0 && f % 300 === 0) console.log(`worker0 ${f - from}/${to - from} ${((performance.now() - t0) / 1000).toFixed(0)}s`);
      }
      ff.stdin.end();
      const code = await ff.exited;
      if (code !== 0) throw new Error(`ffmpeg segment ${w} exited ${code}: ${(await stderr).trim()}`);
      await page.close();
    }),
  );
  const list = join(paths.frames, "list.txt");
  const videoOnly = join(paths.frames, "video.mp4");
  await Bun.write(list, Array.from({ length: workers }, (_, w) => `file 'seg_${w}.mp4'`).join("\n") + "\n");
  await exec(["ffmpeg", "-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", list, "-c", "copy", videoOnly]);

  // narration track: each clip placed at scene start + lead
  const narration = join(paths.frames, "narration.wav");
  const normalized = join(paths.frames, "narration_norm.wav");
  const inputs = tl.scenes.flatMap((s) => ["-i", join(paths.audio, `${s.id}.mp3`)]);
  const delays = tl.scenes.map((s, i) => `[${i}:a]aresample=48000,adelay=${Math.round((s.start + s.lead) * 1000)}:all=1[a${i}]`).join(";");
  const mix = `${delays};${tl.scenes.map((_, i) => `[a${i}]`).join("")}amix=inputs=${tl.scenes.length}:normalize=0,apad,atrim=0:${tl.total.toFixed(3)}[mix]`;
  await exec(["ffmpeg", "-y", "-loglevel", "error", ...inputs, "-filter_complex", mix, "-map", "[mix]", "-ac", "2", "-ar", "48000", narration]);
  const pass1 = await exec(["ffmpeg", "-hide_banner", "-i", narration, "-af", "loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json", "-f", "null", "-"]);
  const json = pass1.stderr.match(/\{[^{}]*"input_i"[^{}]*\}/)?.[0];
  if (!json) throw new Error(`loudnorm pass 1 printed no measurement: ${pass1.stderr.slice(-400)}`);
  const m = Loudnorm(JSON.parse(json));
  if (m instanceof type.errors) throw new Error(`loudnorm pass 1: ${m.summary}`);
  const ln = `loudnorm=I=-14:TP=-1.5:LRA=11:measured_I=${m.input_i}:measured_TP=${m.input_tp}:measured_LRA=${m.input_lra}:measured_thresh=${m.input_thresh}:offset=${m.target_offset}:linear=true`;
  await exec(["ffmpeg", "-y", "-loglevel", "error", "-i", narration, "-af", ln, "-ar", "48000", normalized]);
  const final = join(paths.out, "final.mp4");
  await exec(["ffmpeg", "-y", "-loglevel", "error", "-i", videoOnly, "-i", normalized, "-map", "0:v", "-map", "1:a",
    "-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart", final]);
  console.log(`done in ${((performance.now() - t0) / 1000).toFixed(0)}s: ${final}`);
}

runMain(async () => {
  const [mode, dir, workersArg, ...rest] = Bun.argv.slice(2);
  const usage = "usage: bun render.ts check|stills <projectDir> | bun render.ts video <projectDir> [workers]";
  if (!dir || rest.length || (mode !== "check" && mode !== "stills" && mode !== "video")) throw new Error(usage);
  if (workersArg !== undefined && mode !== "video") throw new Error(usage);
  const workers = Number(workersArg ?? 6);
  if (!Number.isInteger(workers) || workers < 1) throw new Error(`workers must be a positive integer, got ${workersArg}`);

  const paths = projectPaths(dir);
  if (!existsSync(paths.scenes)) throw new Error(`missing scenes.js: ${paths.scenes}`);
  const tl = await loadTimeline(paths, await loadNarration(paths));
  if (mode === "video") for (const s of tl.scenes) if (!existsSync(join(paths.audio, `${s.id}.mp3`))) throw new Error(`missing clip audio/${s.id}.mp3`);
  await ensureFont();
  const chrome = findChrome();
  console.log(`chrome: chrome-headless-shell build ${chrome.rev}`);

  const server = serve(paths);
  const url = `http://127.0.0.1:${server.port}/`;
  const browser = await chromium.launch({ executablePath: chrome.exe, args: ["--disable-gpu", "--font-render-hinting=none"] });
  try {
    if (mode === "video") return await video(browser, url, paths, tl, workers);
    const { page, errors } = await openPage(browser, url);
    await check(page, errors, tl);
    if (mode === "stills") await stills(page, paths, tl);
  } finally {
    await browser.close();
    server.stop(true);
  }
});
