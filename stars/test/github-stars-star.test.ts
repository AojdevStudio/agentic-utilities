import { expect, test } from "bun:test";
import { type GitHubCall, mergeStars, parseStarRef, starRepositories } from "../src/github-stars-lib.ts";

function synced(name: string) {
  const { ledger } = mergeStars(
    null,
    "example-user",
    [
      {
        starred_at: "2026-09-25T12:00:00Z",
        repo: {
          archived: false,
          description: "Tool",
          full_name: name,
          html_url: `https://github.com/${name}`,
          language: "TypeScript",
          pushed_at: "2026-09-24T12:00:00Z",
          stargazers_count: 1,
        },
      },
    ],
    new Map(),
  );
  return { ledger };
}

test("star references require an exact GitHub repository URL or owner/repo", () => {
  expect(parseStarRef("https://github.com/Owner/Repo")).toBe("Owner/Repo");
  expect(parseStarRef("https://github.com/Owner/Repo.git")).toBe("Owner/Repo");
  expect(parseStarRef("git@github.com:Owner/Repo.git")).toBe("Owner/Repo");
  expect(parseStarRef("Owner/Repo")).toBe("Owner/Repo");
  expect(() => parseStarRef("Repo")).toThrow();
  expect(() => parseStarRef("https://github.com/Owner/Repo/issues")).toThrow();
  expect(() => parseStarRef("https://github.com.evil.test/Owner/Repo")).toThrow();
});

test("resolves canonical identity, stars once, and syncs the existing workflow", () => {
  const calls: string[] = [];
  const gh: GitHubCall = (args) => {
    calls.push(args.join(" "));
    if (args[1] === "repos/old/name")
      return {
        status: 0,
        stdout: JSON.stringify({ full_name: "new/name", html_url: "https://github.com/new/name" }),
        stderr: "",
      };
    if (args[1] === "user/starred/new/name" && args.includes("GET"))
      return { status: 1, stdout: "", stderr: "gh: Not Found (HTTP 404)" };
    return { status: 0, stdout: "", stderr: "" };
  };
  const results = starRepositories(["old/name"], gh, () => synced("new/name"));
  expect(results).toEqual([
    {
      requested: "old/name",
      fullName: "new/name",
      url: "https://github.com/new/name",
      status: "starred",
      workflow: { status: "synced" },
    },
  ]);
  expect(calls).toContain("api user/starred/new/name -X PUT --silent");
});

test("already starred repository is reported without another PUT", () => {
  const calls: string[] = [];
  const gh: GitHubCall = (args) => {
    calls.push(args.join(" "));
    if (args[1] === "repos/org/repo")
      return {
        status: 0,
        stdout: JSON.stringify({ full_name: "org/repo", html_url: "https://github.com/org/repo" }),
        stderr: "",
      };
    return { status: 0, stdout: "", stderr: "" };
  };
  const results = starRepositories(["https://github.com/org/repo"], gh, () => synced("org/repo"));
  expect(results[0]?.status).toBe("already-starred");
  expect(calls.some((call) => call.includes(" PUT "))).toBe(false);
});

test("batch retains successes and failures and reports sync failure separately", () => {
  const calls: string[] = [];
  const gh: GitHubCall = (args) => {
    calls.push(args.join(" "));
    if (args[1] === "repos/bad/missing") return { status: 1, stdout: "", stderr: "gh: Not Found (HTTP 404)" };
    if (args[1]?.startsWith("repos/")) {
      const name = args[1].slice("repos/".length);
      return {
        status: 0,
        stdout: JSON.stringify({ full_name: name, html_url: `https://github.com/${name}` }),
        stderr: "",
      };
    }
    if (args.includes("GET")) return { status: 1, stdout: "", stderr: "gh: Not Found (HTTP 404)" };
    if (args[1] === "user/starred/bad/write") return { status: 1, stdout: "", stderr: "gh: Forbidden (HTTP 403)" };
    return { status: 0, stdout: "", stderr: "" };
  };
  const results = starRepositories(["good/one", "bad/missing", "bad/write", "good/two"], gh, () => {
    throw new Error("sync offline");
  });
  expect(results.map((result) => result.status)).toEqual(["starred", "failed", "failed", "starred"]);
  expect(results[0]?.workflow).toEqual({ status: "failed", error: "sync offline" });
  expect(results[1]?.workflow).toEqual({ status: "skipped" });
  expect(results[2]?.error).toContain("403");
  expect(results[3]?.workflow).toEqual({ status: "failed", error: "sync offline" });
  expect(calls.filter((call) => call.includes(" PUT ")).length).toBe(3);
});
