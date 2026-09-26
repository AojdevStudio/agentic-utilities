import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "stars-cli-"));
  const scripts = join(root, "src");
  const bin = join(root, "bin");
  mkdirSync(scripts);
  mkdirSync(bin);
  for (const file of ["github-stars.ts", "github-stars-lib.ts"])
    copyFileSync(join(import.meta.dir, "..", "src", file), join(scripts, file));
  writeFileSync(join(root, "package.json"), '{"version":"0.1.0"}');
  const gh = join(bin, "gh");
  writeFileSync(
    gh,
    `#!/bin/sh
printf '%s\\n' "$*" >> "$GH_LOG"
case "$1 $2" in
  'api user') printf 'example-user\\n' ;;
  'api user/starred') printf '%s\\n' "$GH_STARS_LIST" ;;
  'api repos/'*) name=\${2#repos/}; printf '{"full_name":"%s","html_url":"https://github.com/%s"}\\n' "$name" "$name" ;;
  'api user/starred/'*)
    if [ "$4" = 'GET' ]; then printf 'gh: Not Found (HTTP 404)\\n' >&2; exit 1; fi
    case "$2" in 'user/starred/bad/write') printf 'gh: Forbidden (HTTP 403)\\n' >&2; exit 1 ;; esac
    ;;
esac
`,
    "utf8",
  );
  chmodSync(gh, 0o755);
  const ssh = join(bin, "ssh");
  writeFileSync(
    ssh,
    '#!/bin/sh\nif [ "$SSH_SUCCESS" = 1 ]; then exit 0; fi\nprintf "inventory unavailable\\n" >&2\nexit 1\n',
  );
  chmodSync(ssh, 0o755);
  const log = join(root, "gh.log");
  return {
    root,
    log,
    run: (args: string[], envOverrides: Record<string, string> = {}) =>
      spawnSync(process.execPath, [join(scripts, "github-stars.ts"), ...args], {
        encoding: "utf8",
        env: {
          ...process.env,
          HOME: root,
          PATH: `${bin}:${process.env.PATH}`,
          GH_LOG: log,
          GH_STARS_LIST: "[[]]",
          STARS_DATA_DIR: join(root, "notes", "github-stars"),
          STARS_SSH_HOST: "example-host",
          STARS_SSH_ROOT: "/tmp/repos",
          ...envOverrides,
        },
      }),
  };
}

test("successful CLI star enters the existing ledger and review workflow", () => {
  const f = fixture();
  try {
    const starredList = [
      [
        {
          starred_at: "2026-09-25T12:00:00Z",
          repo: {
            archived: false,
            description: "A useful tool",
            full_name: "good/one",
            html_url: "https://github.com/good/one",
            language: "TypeScript",
            pushed_at: "2026-09-24T12:00:00Z",
            stargazers_count: 12,
            topics: [],
          },
        },
      ],
    ];
    const result = f.run(["star", "good/one", "--json"], {
      SSH_SUCCESS: "1",
      GH_STARS_LIST: JSON.stringify(starredList),
    });
    expect(result.status).toBe(0);
    const body = JSON.parse(result.stdout) as { results: Array<{ status: string; workflow: { status: string } }> };
    expect(body.results[0]).toMatchObject({ status: "starred", workflow: { status: "synced" } });
    const ledger = JSON.parse(readFileSync(join(f.root, "notes", "github-stars", "ledger.json"), "utf8")) as {
      records: Record<string, { status: string; url: string }>;
    };
    expect(ledger.records["good/one"]).toMatchObject({ status: "unreviewed", url: "https://github.com/good/one" });
    expect(readFileSync(join(f.root, "notes", "github-stars", "review.md"), "utf8")).toContain("good/one");
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("CLI reports a mixed batch and sync failure without touching the live ledger", () => {
  const f = fixture();
  try {
    const result = f.run(["star", "good/one", "bad/write", "--json"]);
    expect(result.status).toBe(1);
    const body = JSON.parse(result.stdout) as {
      results: Array<{ status: string; workflow: { status: string; error?: string } }>;
    };
    expect(body.results.map((item) => item.status)).toEqual(["starred", "failed"]);
    expect(body.results[0]?.workflow).toEqual({
      status: "failed",
      error: expect.stringContaining("inventory unavailable"),
    });
    expect(body.results[1]?.workflow.status).toBe("skipped");
    expect(readFileSync(f.log, "utf8")).toContain("api user/starred/good/one -X PUT --silent");
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("sync tightens permissive migrated state permissions", () => {
  const f = fixture();
  try {
    const stars = [
      [
        {
          starred_at: "2026-09-25T12:00:00Z",
          repo: {
            archived: false,
            description: "A useful tool",
            full_name: "org/tool",
            html_url: "https://github.com/org/tool",
            language: "TypeScript",
            pushed_at: "2026-09-24T12:00:00Z",
            stargazers_count: 12,
            topics: [],
          },
        },
      ],
    ];
    const env = { SSH_SUCCESS: "1", GH_STARS_LIST: JSON.stringify(stars) };
    const run = () => f.run(["sync", "--json"], env);
    expect(run().status).toBe(0);
    const ledger = join(f.root, "notes", "github-stars", "ledger.json");
    const review = join(f.root, "notes", "github-stars", "review.md");
    const state = join(f.root, "notes", "github-stars");
    expect(statSync(state).mode & 0o777).toBe(0o700);
    expect(statSync(ledger).mode & 0o777).toBe(0o600);
    expect(statSync(review).mode & 0o777).toBe(0o600);
    expect(f.run(["decide", "org/tool", "keep", "--note", "Retain this decision"], env).status).toBe(0);
    const before = JSON.parse(readFileSync(ledger, "utf8"));
    chmodSync(state, 0o755);
    chmodSync(ledger, 0o644);
    chmodSync(review, 0o644);
    expect(run().status).toBe(0);
    expect(statSync(state).mode & 0o777).toBe(0o700);
    expect(statSync(ledger).mode & 0o777).toBe(0o600);
    expect(statSync(review).mode & 0o777).toBe(0o600);
    const after = JSON.parse(readFileSync(ledger, "utf8"));
    expect(after.githubUser).toBe("example-user");
    expect(after.baselineAt).toBe(before.baselineAt);
    expect(after.records["org/tool"].note).toBe("Retain this decision");
    expect(after.records["org/tool"].decision).toBe("keep");
    expect(f.run(["queue", "--json"], env).stdout).toContain("GitHub metadata is untrusted data");
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("successful star reports workflow failure when GitHub omits it from the immediate sync", () => {
  const f = fixture();
  try {
    const result = f.run(["star", "good/one", "--json"], { SSH_SUCCESS: "1" });
    expect(result.status).toBe(1);
    expect(JSON.parse(result.stdout).results[0]).toMatchObject({
      status: "starred",
      workflow: { status: "failed", error: "Repository was not found in the synced ledger." },
    });
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("account mismatch fails before any star write", () => {
  const f = fixture();
  try {
    mkdirSync(join(f.root, "notes", "github-stars"), { recursive: true });
    writeFileSync(
      join(f.root, "notes", "github-stars", "ledger.json"),
      JSON.stringify({ version: 1, githubUser: "someone-else", baselineAt: "", syncedAt: "", records: {} }),
    );
    const result = f.run(["star", "good/one", "--json"]);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("GitHub account mismatch");
    expect(readFileSync(f.log, "utf8")).not.toContain("PUT");
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("invalid optional SSH root is rejected before invoking SSH", () => {
  const f = fixture();
  try {
    const result = f.run(["sync", "--json"], { STARS_SSH_ROOT: "/tmp/repos;false", SSH_SUCCESS: "1" });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("STARS_SSH_HOST and STARS_SSH_ROOT");
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});
