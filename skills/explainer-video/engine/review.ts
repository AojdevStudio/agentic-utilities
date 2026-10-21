// Second-model review of a project's stills with Codex.
//   bun review.ts <projectDir> [--model gpt-6.1-sol]
// Writes review<N>.md (Codex's answer) and review<N>.log (its full output), N = next unused round.
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { loadNarration, loadTimeline, projectPaths, runMain, stillName, stillTimes } from "./contract.ts";

const TIMEOUT_MS = 900_000;

runMain(async () => {
  const { values, positionals } = parseArgs({
    args: Bun.argv.slice(2),
    options: { model: { type: "string", default: "gpt-6.1-sol" } },
    allowPositionals: true,
  });
  const [dir] = positionals;
  if (!dir || positionals.length > 1) throw new Error("usage: bun review.ts <projectDir> [--model <codex model>]");
  const paths = projectPaths(dir);
  const tl = await loadTimeline(paths, await loadNarration(paths));

  const names = tl.scenes.flatMap((s) => stillTimes(s).map((_, n) => stillName(s.id, n)));
  const missing = names.filter((n) => !existsSync(join(paths.stills, n)));
  if (missing.length) throw new Error(`missing stills (run render.ts stills): ${missing.join(", ")}`);
  if (!existsSync(paths.brief)) throw new Error(`missing brief.md: ${paths.brief}`);
  const brief = (await Bun.file(paths.brief).text()).trim();
  if (!brief) throw new Error(`${paths.brief} is empty`);

  const template = await Bun.file(join(import.meta.dir, "review-prompt.md")).text();
  const prompt = template
    .replace("{{STILLS}}", names.map((n) => n.replace(/\.png$/, "")).join(", "))
    .replace("{{BRIEF}}", brief);

  const rounds = readdirSync(paths.root).map((f) => Number(f.match(/^review(\d+)\.md$/)?.[1] ?? 0));
  const round = Math.max(0, ...rounds) + 1;
  const out = join(paths.root, `review${round}.md`);
  const log = join(paths.root, `review${round}.log`);

  if (!Bun.which("codex")) throw new Error("codex is not on PATH");
  // The prompt must come after `--`: otherwise codex reads it as more -i image paths.
  const cmd = ["codex", "exec", "--skip-git-repo-check", "-m", values.model, "-s", "read-only", "-o", out,
    ...names.flatMap((n) => ["-i", join(paths.stills, n)]), "--", prompt];
  console.log(`review ${round}: ${names.length} stills to ${values.model}`);
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe", timeout: TIMEOUT_MS });
  const [stdout, stderr, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  await Bun.write(log, stdout + stderr);
  if (proc.signalCode) throw new Error(`codex was killed (${proc.signalCode}) after ${TIMEOUT_MS / 1000}s. Log: ${log}`);
  if (code !== 0) throw new Error(`codex exited ${code}. Log: ${log}\n${stderr.trim().slice(-1500)}`);
  const text = existsSync(out) ? (await Bun.file(out).text()).trim() : "";
  if (!text) throw new Error(`codex exited 0 but wrote no review to ${out}. Log: ${log}`);

  console.log(`\n${text}\n`);
  console.log(text === "CLEAN" ? "review: CLEAN" : `review: problems found, see ${out}`);
});
