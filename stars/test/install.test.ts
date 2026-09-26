import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const packageRoot = join(import.meta.dir, "..");

test("tarball installs and runs outside the checkout with user-local state", () => {
  const home = mkdtempSync(join(tmpdir(), "stars-install-"));
  try {
    const packed = spawnSync(process.execPath, ["pm", "pack", "--destination", home], {
      cwd: packageRoot,
      encoding: "utf8",
    });
    expect(packed.status).toBe(0);
    const tarball = join(home, "aoj-stars-0.1.0.tgz");
    const contents = spawnSync("tar", ["-tzf", tarball], { encoding: "utf8" });
    expect(contents.status).toBe(0);
    expect(contents.stdout.trim().split("\n").sort()).toEqual(
      [
        "package/LICENSE",
        "package/README.md",
        "package/package.json",
        "package/src/github-stars-lib.ts",
        "package/src/github-stars.ts",
      ].sort(),
    );

    const bunInstall = join(home, "bun");
    const env = {
      ...process.env,
      HOME: home,
      XDG_STATE_HOME: join(home, "state"),
      BUN_INSTALL: bunInstall,
    };
    const install = spawnSync(process.execPath, ["add", "--global", tarball], { cwd: home, env, encoding: "utf8" });
    expect(install.status).toBe(0);
    const executable = join(bunInstall, "bin", "stars");
    const run = (args: string[], extraEnv: Record<string, string> = {}) =>
      spawnSync(executable, args, {
        cwd: home,
        encoding: "utf8",
        env: { ...env, ...extraEnv },
      });
    expect(run(["--version"]).stdout).toBe("stars 0.1.0\n");
    expect(run(["--help"]).stdout).toContain("stars star OWNER/REPO");

    const bin = join(home, "mock-bin");
    mkdirSync(bin);
    const gh = join(bin, "gh");
    writeFileSync(
      gh,
      `#!/bin/sh
case "$1 $2" in
  'api user') printf 'example-user\\n' ;;
  'api user/starred') printf '%s\\n' "$GH_STARS_LIST" ;;
  'api repos/'*) name=\${2#repos/}; printf '{"full_name":"%s","html_url":"https://github.com/%s"}\\n' "$name" "$name" ;;
  'api user/starred/'*) if [ "$4" = 'GET' ]; then printf 'gh: Not Found (HTTP 404)\\n' >&2; exit 1; fi ;;
esac
`,
    );
    chmodSync(gh, 0o755);
    const stars = [
      [
        {
          starred_at: "2026-09-25T12:00:00Z",
          repo: {
            archived: false,
            description: "A useful tool",
            full_name: "example/one",
            html_url: "https://github.com/example/one",
            language: "TypeScript",
            pushed_at: "2026-09-24T12:00:00Z",
            stargazers_count: 12,
            topics: [],
          },
        },
      ],
    ];
    const result = run(["star", "example/one", "--json"], {
      PATH: `${bin}:${process.env.PATH}`,
      GH_STARS_LIST: JSON.stringify(stars),
    });
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout).results[0]).toMatchObject({
      fullName: "example/one",
      url: "https://github.com/example/one",
      status: "starred",
      workflow: { status: "synced" },
    });
    const ledger = JSON.parse(readFileSync(join(home, "state", "stars", "ledger.json"), "utf8"));
    expect(ledger.records["example/one"].url).toBe("https://github.com/example/one");
    expect(readFileSync(join(home, "state", "stars", "review.md"), "utf8")).toContain("example/one");
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
