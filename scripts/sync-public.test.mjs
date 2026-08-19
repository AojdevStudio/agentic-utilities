import { test } from "bun:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  applyReplacements,
  dropFrontmatterKeys,
  dropLines,
  expandHome,
  findExcludedRefs,
  findLeaks,
  globToRegExp,
  matchesAnyGlob,
  mergeLocalOverlay,
  rewriteSkillName,
  transformText,
} from "./sync-public.mjs";

const manifest = JSON.parse(await readFile(new URL("../public-manifest.json", import.meta.url), "utf8"));
const { transforms, leakPatterns } = manifest;

// The tracked manifest carries structural patterns only. Private-term patterns
// live in the gitignored overlay, so they are exercised here through a synthetic
// overlay rather than by naming real machines in a public test file.
const SYNTHETIC_OVERLAY = {
  leakPatterns: [
    { id: "private-machine-names", description: "example", regex: "\\b(?:example-host|example-box)\\b" },
    { id: "private-repo-names", description: "example", regex: "\\b(?:example-vault)\\b" },
  ],
  transforms: { replace: [{ find: "example-vault", replaceWith: "a private repo" }] },
};

// ---- glob matching ---------------------------------------------------------

test("`**` crosses separators, `*` does not", () => {
  assert.ok(globToRegExp("agents/**").test("agents/openai.yaml"));
  assert.ok(globToRegExp("scripts/**").test("scripts/nested/deep.mjs"));
  assert.ok(!globToRegExp("scripts/*").test("scripts/nested/deep.mjs"));
  assert.ok(globToRegExp("scripts/*").test("scripts/run.mjs"));
});

test("literal dots in a glob do not act as regex wildcards", () => {
  assert.ok(globToRegExp("protocols.md").test("protocols.md"));
  assert.ok(!globToRegExp("protocols.md").test("protocolsXmd"));
});

test("matchesAnyGlob tolerates an absent glob list", () => {
  assert.equal(matchesAnyGlob("SKILL.md", undefined), false);
  assert.equal(matchesAnyGlob("SKILL.md", []), false);
});

// ---- home expansion --------------------------------------------------------

test("expandHome rewrites only a leading tilde segment", () => {
  assert.equal(expandHome("~/store", "/home/x"), "/home/x/store");
  assert.equal(expandHome("~", "/home/x"), "/home/x");
  assert.equal(expandHome("/abs/path", "/home/x"), "/abs/path");
  assert.equal(expandHome("./rel/~/path", "/home/x"), "./rel/~/path");
});

// ---- frontmatter surgery ---------------------------------------------------

const STORE_SKILL = `---
name: find-docs
description: Fetch current docs. USE WHEN the user asks about a library.
metadata:
  category: engineering
  lanes: [claude, codex, pi]
  machines: [example-host]
  requires: [ctx7]
---

# Documentation Lookup

Body text.
`;

test("dropFrontmatterKeys removes only the declared keys", () => {
  const out = dropFrontmatterKeys(STORE_SKILL, transforms.dropFrontmatterKeys);
  assert.ok(!out.includes("machines"));
  assert.ok(!out.includes("example-host"));
  assert.ok(!out.includes("requires"));
  assert.ok(out.includes("category: engineering"));
  assert.ok(out.includes("lanes:"));
  assert.ok(out.includes("# Documentation Lookup"));
  assert.ok(out.includes("Body text."));
});

test("an emptied parent mapping is removed rather than left as `metadata: {}`", () => {
  const input = `---
name: x
metadata:
  machines: [example-host]
  requires: [ctx7]
---

Body.
`;
  const out = dropFrontmatterKeys(input, transforms.dropFrontmatterKeys);
  assert.ok(!out.includes("metadata"));
  assert.ok(out.includes("name: x"));
  assert.ok(out.includes("Body."));
});

test("files without frontmatter pass through byte-identical", () => {
  const prose = "# A reference file\n\nNo frontmatter here.\n";
  assert.equal(dropFrontmatterKeys(prose, transforms.dropFrontmatterKeys), prose);
  assert.equal(rewriteSkillName(prose, "anything"), prose);
});

test("a `---` inside the body does not truncate the frontmatter block", () => {
  const input = `---
name: x
metadata:
  machines: [example-host]
---

Body with a rule:

---

More body.
`;
  const out = dropFrontmatterKeys(input, transforms.dropFrontmatterKeys);
  assert.ok(out.includes("More body."));
  assert.ok(!out.includes("example-host"));
});

test("rewriteSkillName forces the public directory name", () => {
  const out = rewriteSkillName("---\nname: git-workflow\ndescription: d\n---\n\nBody.\n", "gitworkflow");
  assert.ok(out.includes("name: gitworkflow"));
  assert.ok(!out.includes("name: git-workflow"));
  assert.ok(out.includes("Body."));
});

// ---- replacements ----------------------------------------------------------

test("declared replacements run in manifest order", () => {
  const out = applyReplacements("Run ~/.claude/skills and ask Ossie about Ossie's setup.", transforms.replace);
  assert.equal(out, "Run $AGENT_HOME/skills and ask the user about the user's setup.");
});

// ---- leak assertion --------------------------------------------------------

test("structural leak patterns catch what the transforms miss", () => {
  const cases = [
    ["/Users/someone/Projects/x", "macos-home-path"],
    ["curl http://192.168.1.44:8080", "rfc1918-ipv4"],
  ];
  for (const [content, expectedId] of cases) {
    const findings = findLeaks(content, leakPatterns);
    assert.ok(findings.length > 0, `expected a finding for: ${content}`);
    assert.ok(
      findings.some((finding) => finding.patternId === expectedId),
      `expected ${expectedId} for: ${content}, got ${findings.map((f) => f.patternId).join(",")}`,
    );
  }
});

test("the tracked manifest names no private machines, repos, or domains", () => {
  // The whole point of the overlay split: this file ships publicly, so an
  // enumeration of private terms must not survive in it.
  const raw = JSON.stringify(manifest);
  for (const id of ["private-machine-names", "private-repo-names", "private-email", "private-framework-paths"]) {
    assert.ok(!raw.includes(id), `${id} belongs in the gitignored overlay, not the tracked manifest`);
  }
  assert.ok(manifest.localOverlay, "the tracked manifest must point at its overlay file");
});

test("overlay leak patterns are appended and overlay replacements run last", () => {
  const merged = mergeLocalOverlay(manifest, SYNTHETIC_OVERLAY);
  assert.deepEqual(
    merged.leakPatterns.map((p) => p.id),
    [...leakPatterns.map((p) => p.id), "private-machine-names", "private-repo-names"],
  );
  assert.deepEqual(
    merged.transforms.replace.map((r) => r.find),
    [...transforms.replace.map((r) => r.find), "example-vault"],
  );
  // Structural patterns survive the merge.
  assert.ok(findLeaks("/Users/someone/x", merged.leakPatterns).length > 0);
});

test("merged private-term patterns fire on overlay terms", () => {
  const merged = mergeLocalOverlay(manifest, SYNTHETIC_OVERLAY);
  for (const [content, expectedId] of [
    ["ssh me@example-host", "private-machine-names"],
    ["see the example-vault repo", "private-repo-names"],
  ]) {
    assert.ok(
      findLeaks(content, merged.leakPatterns).some((f) => f.patternId === expectedId),
      `expected ${expectedId} for: ${content}`,
    );
  }
});

test("mergeLocalOverlay with no overlay returns the manifest unchanged", () => {
  assert.equal(mergeLocalOverlay(manifest, null), manifest);
});

test("documented placeholder home paths are not flagged", () => {
  assert.deepEqual(findLeaks("/Users/you/Projects/demo", leakPatterns), []);
  assert.deepEqual(findLeaks("/Users/username/skills", leakPatterns), []);
});

test("bare ~/.claude is normalized, not gated", () => {
  // It is the documented Claude Code config directory, not private. Gating it
  // would fire on every harness-path mention and train hook bypasses. The
  // transform still rewrites it for portability.
  assert.deepEqual(findLeaks("cd ~/.claude/skills", leakPatterns), []);
  assert.equal(applyReplacements("cd ~/.claude/skills", transforms.replace), "cd $AGENT_HOME/skills");
});

test("semver strings are not mistaken for private addresses", () => {
  // Regression: a three-part 10/8 branch matched every "10.0.2" in a lockfile,
  // which is how this gate first fired on diataxis-docs-site's package-lock.
  for (const version of ['"version": "10.0.2"', "node@10.8.2", "^192.168.0", "172.16.4"]) {
    assert.deepEqual(findLeaks(version, leakPatterns), [], `flagged: ${version}`);
  }
  for (const address of ["10.0.2.15", "192.168.1.44", "172.16.4.9"]) {
    assert.ok(
      findLeaks(address, leakPatterns).some((f) => f.patternId === "rfc1918-ipv4"),
      `missed: ${address}`,
    );
  }
});

test("leak findings carry a line number for the report", () => {
  const merged = mergeLocalOverlay(manifest, SYNTHETIC_OVERLAY);
  const [finding] = findLeaks("clean line\nanother\nssh example-host\n", merged.leakPatterns);
  assert.equal(finding.line, 3);
  assert.equal(finding.patternId, "private-machine-names");
  assert.equal(finding.excerpt, "example-host");
});

// ---- golden: the whole chain ----------------------------------------------

test("golden — a store SKILL.md transforms into publishable output", () => {
  const out = transformText(STORE_SKILL, { publicName: "find-docs", transforms, isSkillManifest: true });
  assert.equal(
    out,
    `---
name: find-docs
description: Fetch current docs. USE WHEN the user asks about a library.
metadata:
  category: engineering
  lanes: [claude, codex, pi]
---

# Documentation Lookup

Body text.
`,
  );
  assert.deepEqual(findLeaks(out, leakPatterns), []);
});

test("golden — a renamed skill keeps its body and gains the public name", () => {
  const input = `---
name: git-workflow
description: Commit and branch. USE WHEN Ossie asks to commit.
metadata:
  machines: [example-host]
---

Templates live in ~/.claude/skills/git-workflow.
`;
  const out = transformText(input, { publicName: "gitworkflow", transforms, isSkillManifest: true });
  assert.equal(
    out,
    `---
name: gitworkflow
description: Commit and branch. USE WHEN the user asks to commit.
---

Templates live in $AGENT_HOME/skills/git-workflow.
`,
  );
  assert.deepEqual(findLeaks(out, leakPatterns), []);
});

test("non-SKILL.md files skip frontmatter surgery but still get replacements", () => {
  const merged = mergeLocalOverlay(manifest, SYNTHETIC_OVERLAY);
  const input = "---\nname: not-a-skill\nmetadata:\n  machines: [example-host]\n---\n\nAsk Ossie.\n";
  const out = transformText(input, { publicName: "find-docs", transforms, isSkillManifest: false });
  assert.ok(out.includes("machines: [example-host]"), "reference files keep their own frontmatter");
  assert.ok(out.includes("Ask the user."));
  // The leak gate is what stops this from shipping, not the transform.
  assert.ok(findLeaks(out, merged.leakPatterns).some((f) => f.patternId === "private-machine-names"));
});

// ---- exclusion bookkeeping -------------------------------------------------

test("dropLines removes matching lines and keeps the rest", () => {
  const table = [
    "| **Commit** | c | workflows/Commit.md |",
    "| **SetIdentity** | i | workflows/SetIdentity.md |",
    "| **Release** | r | workflows/Release.md |",
  ].join("\n");
  const out = dropLines(table, ["\\|\\s*\\*\\*SetIdentity\\*\\*\\s*\\|"]);
  assert.ok(!out.includes("SetIdentity"));
  assert.ok(out.includes("**Commit**"));
  assert.ok(out.includes("**Release**"));
});

test("a reference to an excluded file is reported", () => {
  const content = "See `workflows/SetIdentity.md` for identity setup.";
  assert.deepEqual(findExcludedRefs(content, ["workflows/SetIdentity.md"]), ["workflows/SetIdentity.md"]);
});

test("excluding one file does not flag its siblings", () => {
  // Regression: deriving the parent directory as a needle made every
  // `workflows/...` mention in the skill look like a dangling reference.
  const content = "See `workflows/Commit.md` and `workflows/Release.md`.";
  assert.deepEqual(findExcludedRefs(content, ["workflows/SetIdentity.md"]), []);
});

test("house-convention prose is not mistaken for a bundle reference", () => {
  // Regression: `tools/**` as a directory needle flagged "tools -> `tools/<name>`",
  // which describes the audited repo's layout, not this skill's files.
  const content = "House path conventions: docs -> `docs/<name>`; tools -> `tools/<name>`.";
  assert.deepEqual(findExcludedRefs(content, ["tools/changelog/changelog"]), []);
});

// ---- manifest integrity ----------------------------------------------------

test("every manifest skill declares a known mode, and forks explain themselves", () => {
  const modes = new Set(["mirror", "forked", "public-owned"]);
  for (const [name, entry] of Object.entries(manifest.skills)) {
    assert.ok(modes.has(entry.mode), `${name}: unknown mode '${entry.mode}'`);
    if (entry.mode !== "mirror") {
      assert.ok(entry.reason, `${name}: '${entry.mode}' entries must record a reason`);
    }
  }
});

test("every leak pattern compiles and carries an id", () => {
  for (const pattern of leakPatterns) {
    assert.ok(pattern.id, "leak pattern is missing an id");
    assert.doesNotThrow(() => new RegExp(pattern.regex), `${pattern.id}: regex does not compile`);
  }
});
