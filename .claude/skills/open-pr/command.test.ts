import { expect, test } from "bun:test";
import { type BodySource, findPrCommands } from "./command";

const target = (command: string) => {
  const [cmd] = findPrCommands(command);
  return cmd?.kind === "merge" ? cmd.target : "not a merge";
};

test.each([
  ["gh pr merge -m 67", "67"],
  ["gh pr merge -r 67", "67"],
  ["gh pr merge -s -d 67", "67"],
  ["gh pr merge 67 -m", "67"],
  ["gh pr merge --match-head-commit abc123 67 --squash", "67"],
  ["gh pr merge --author-email agent@example.com 67 --squash", "67"],
  ["gh pr merge -A agent@example.com 67", "67"],
  ["gh pr merge -Aagent@example.com 67", "67"],
  ["gh pr merge -tSubject 67 --squash", "67"],
  ["gh pr merge -b 'merge body' 67", "67"],
  ["gh pr merge 67 --match-head-commit abc123", "67"],
  ["gh pr merge -st feat/x 67", "67"],
  ["gh pr merge -dA x 67", "67"],
  ["gh pr merge -sR o/r 67", "67"],
  ["gh pr merge -dtSubject 67", "67"],
  ["gh pr merge -sb text 67", "67"],
])("%s targets PR 67", (command, expected) => {
  expect(target(command)).toBe(expected);
});

const body = (command: string): BodySource | undefined => {
  const [cmd] = findPrCommands(command);
  if (cmd?.kind !== "create" && cmd?.kind !== "edit") throw new Error(`not a create or edit: ${command}`);
  return cmd.body;
};

test.each([
  ["gh pr merge -sR o/r 67", "o/r", "67", true],
  ["gh pr merge -R=other/repo 67 --squash", "other/repo", "67", true],
  ["gh pr merge -Rother/repo 67", "other/repo", "67", true],
  ["gh pr merge --repo=other/repo 67", "other/repo", "67", true],
  ["gh pr merge --body -sRother/repo 67 --squash", undefined, "67", false],
  ["gh pr merge --body -ABC 67 --squash", undefined, "67", false],
  ["gh pr merge -t -R 67", undefined, "67", false],
  ["gh pr merge -- 67", undefined, "67", false],
  ["gh pr merge 67 --squash -R o/r 2>&1", "o/r", "67", true],
  ["gh pr merge 67 --squash 2> /dev/null -R o/r", "o/r", "67", true],
  ["gh pr merge -t s 2>/dev/null 67", undefined, "67", false],
  ["gh pr merge --subject 2>/dev/null 7", undefined, undefined, false],
])("%s keeps its repo, target, and pinned status", (command, repo, expected, pinned) => {
  const [cmd] = findPrCommands(command);
  if (cmd?.kind !== "merge") throw new Error(`not a merge: ${command}`);
  expect({ repo: cmd.repo, target: cmd.target, pinned: cmd.pinned }).toEqual({ repo, target: expected, pinned });
});

test.each([
  "gh pr merge -s=t 67",
  "gh pr merge -d=F 67",
  "gh pr merge --squash=true 67",
  "gh pr merge -t= 67",
  "gh pr merge -b= 67 --squash",
  "gh pr merge --repo= 67 -s",
  "gh pr merge -R '' 67",
  "gh pr merge 67 68",
  "gh pr merge --help",
  "gh pr merge -x 67",
  "gh pr -R o/r merge 67 -s",
  "gh pr --repo=o/r merge 67",
  "gh pr -t -R merge 67",
  "gh pr -sRo/r merge 67",
  "gh -R o/r pr merge 67",
  "gh --repo o/r pr create -t t -b b",
  "gh pr -R o/r create -t t -b b",
  "GH_REPO=o/r gh pr merge 67 -s",
  "env GH_REPO=o/r gh pr merge 67",
  "export GH_REPO=o/r; gh pr merge 67",
  "GH_REPO=o/r bash -c 'gh pr merge 67'",
  "export GH_RE''PO=o/r; gh pr merge 67",
  "env G\\H_REPO=o/r gh pr merge 67",
  "GH_HOST=example.com gh pr merge 67 -R o/r",
  "GH_REPO=o/r gh api -X PUT repos/{owner}/{repo}/pulls/67/merge",
])("%s is refused as unreadable", (command) => {
  const [cmd] = findPrCommands(command);
  expect(cmd?.kind).toBe("unreadable");
});

test("a field inside another flag's value is not the body", () => {
  expect(body("gh api -X PATCH repos/o/r/pulls/67 -F body=@bad.md --jq '--raw-field=body=GOOD'")).toEqual({
    kind: "file",
    path: "bad.md",
  });
});

test.each([
  ["create", "gh api repos/o/r/pulls -f title=t"],
  ["edit", "gh api -X PATCH repos/o/r/pulls/67"],
])("a raw-field body=@file on %s is the literal text gh sends", (_kind, prefix) => {
  for (const flag of ["-f body=@body.md", "--raw-field body=@body.md", "--raw-field=body=@body.md", "-fbody=@body.md"]) {
    expect(body(`${prefix} ${flag}`)).toEqual({ kind: "inline", text: "@body.md" });
  }
});

test.each([
  ["create", "gh api repos/o/r/pulls -f title=t"],
  ["edit", "gh api -X PATCH repos/o/r/pulls/67"],
])("a typed-field body=@file on %s reads the file", (_kind, prefix) => {
  for (const flag of ["-F body=@body.md", "--field body=@body.md", "--field=body=@body.md"]) {
    expect(body(`${prefix} ${flag}`)).toEqual({ kind: "file", path: "body.md" });
  }
});
