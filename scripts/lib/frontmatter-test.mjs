import assert from "node:assert/strict";
import { isNonEmptyString, parseFrontmatter } from "./frontmatter.mjs";

const FILE = "fixture.md";

function frontmatterBlock(descriptionLine) {
  return `---\nname: fixture\n${descriptionLine}\n---\n\nBody text.\n`;
}

// Valid `description` values: parseFrontmatter must return exactly the
// expected string, and isNonEmptyString must accept it.
const VALID_DESCRIPTIONS = [
  { label: "plain scalar", line: "description: hello world", expected: "hello world" },
  { label: "double-quoted", line: 'description: "hello world"', expected: "hello world" },
  { label: "single-quoted", line: "description: 'hello world'", expected: "hello world" },
  { label: "folded block scalar", line: "description: >\n  hello\n  world", expected: "hello world\n" },
  { label: "literal block scalar", line: "description: |\n  hello\n  world", expected: "hello\nworld\n" },
];

for (const { label, line, expected } of VALID_DESCRIPTIONS) {
  const frontmatter = parseFrontmatter(frontmatterBlock(line), FILE);
  assert.equal(frontmatter.description, expected, `${label}: unexpected parsed value`);
  assert.ok(isNonEmptyString(frontmatter.description), `${label}: expected to be accepted as non-empty string`);
}

// Invalid `description` values: parseFrontmatter still succeeds (the
// frontmatter block itself is valid YAML), but isNonEmptyString must reject
// the resulting value. These are the exact false-negative shapes the
// hand-rolled tokenizer previously accepted.
const INVALID_DESCRIPTIONS = [
  { label: "YAML null (explicit)", line: "description: null" },
  { label: "YAML null (tilde)", line: "description: ~" },
  { label: "empty sequence", line: "description: []" },
  { label: "empty mapping", line: "description: {}" },
  { label: "comment-only (parses to null)", line: "description: # just a comment" },
  { label: "empty string", line: 'description: ""' },
  { label: "whitespace-only string", line: 'description: "   "' },
  { label: "field absent", line: "other: value" },
];

for (const { label, line } of INVALID_DESCRIPTIONS) {
  const frontmatter = parseFrontmatter(frontmatterBlock(line), FILE);
  assert.ok(!isNonEmptyString(frontmatter.description), `${label}: expected to be rejected as non-empty string`);
}

// Malformed frontmatter blocks: parseFrontmatter itself must throw.
const MALFORMED_BLOCKS = [
  {
    label: "malformed YAML (unterminated quoted string)",
    content: '---\nname: fixture\ndescription: "unterminated\n---\n',
  },
  { label: "missing frontmatter block entirely", content: "# just a heading\n\nno frontmatter here\n" },
  { label: "unterminated frontmatter (no closing ---)", content: "---\nname: fixture\ndescription: hi\n" },
  { label: "frontmatter is a sequence, not a mapping", content: "---\n- one\n- two\n---\n" },
];

for (const { label, content } of MALFORMED_BLOCKS) {
  assert.throws(() => parseFrontmatter(content, FILE), undefined, `${label}: expected parseFrontmatter to throw`);
}

console.log("frontmatter parser fixtures passed");
