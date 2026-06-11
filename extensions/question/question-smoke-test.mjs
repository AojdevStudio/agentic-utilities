import assert from "node:assert/strict";
import { createJiti } from "@mariozechner/jiti";

const jiti = createJiti(import.meta.url);
const extensionModule = await jiti.import("../question.ts");
const types = await jiti.import("./rpiv/tool/types.ts");
const validator = await jiti.import("./rpiv/tool/validate-questionnaire.ts");
const response = await jiti.import("./rpiv/tool/response-envelope.ts");

const valid = {
  questions: [
    {
      question: "Which metadata representation should we use?",
      header: "Metadata",
      options: [
        {
          label: "Manifest (Recommended)",
          description: "Best for production routing and scoring while keeping SKILL.md portable.",
          preview: '```json\n{"lensId":"security"}\n```',
        },
        {
          label: "Pure SKILL.md",
          description: "Best for manual portability, weaker for machine routing.",
        },
      ],
    },
  ],
};

assert.equal(types.MAX_QUESTIONS, 4);
assert.equal(types.MIN_OPTIONS, 2);
assert.equal(types.MAX_OPTIONS, 4);
assert.deepEqual(validator.validateQuestionnaire(valid), { ok: true });

const tooFewOptions = {
  questions: [
    {
      question: "Pick one?",
      header: "Pick",
      options: [{ label: "Only", description: "Not enough alternatives." }],
    },
  ],
};
assert.equal(validator.validateQuestionnaire(tooFewOptions).error, "empty_options");

const reservedLabel = {
  questions: [
    {
      question: "Pick one?",
      header: "Pick",
      options: [
        { label: "Other", description: "Reserved by runtime." },
        { label: "Real option", description: "Allowed option." },
      ],
    },
  ],
};
assert.equal(validator.validateQuestionnaire(reservedLabel).error, "reserved_label");

const tooManyOptions = {
  questions: [
    {
      question: "Pick one?",
      header: "Pick",
      options: ["One", "Two", "Three", "Four", "Five"].map((label) => ({ label, description: `${label} option.` })),
    },
  ],
};
assert.equal(validator.validateQuestionnaire(tooManyOptions).error, "too_many_options");

const envelope = response.buildQuestionnaireResponse(
  {
    cancelled: false,
    answers: [
      {
        questionIndex: 0,
        question: valid.questions[0].question,
        kind: "option",
        answer: "Manifest (Recommended)",
        notes: "Prefer this for Pi.",
        preview: valid.questions[0].options[0].preview,
      },
    ],
  },
  valid,
);

assert.equal(envelope.details.cancelled, false);
assert.equal(envelope.details.answers[0].notes, "Prefer this for Pi.");
assert.match(envelope.content[0].text, /selected preview:/);
assert.match(envelope.content[0].text, /user notes:/);

const tools = [];
extensionModule.default({
  registerTool(tool) {
    tools.push(tool);
  },
  getCommands() {
    return [];
  },
  sendMessage() {},
  events: { emit() {} },
});
assert.deepEqual(
  tools.map((tool) => tool.name),
  ["ask_user_question", "AskUserQuestion", "AskBatchQuestions"],
);

const batchTool = tools.find((tool) => tool.name === "AskBatchQuestions");
const noUiResult = await batchTool.execute(
  "tool-call-id",
  {
    title: "TUI no UI smoke",
    presentation: "tui",
    questions: valid.questions.map((question) => ({
      question: question.question,
      type: "decision",
      options: question.options,
      recommendedOption: question.options[0].label,
      recommendation: "Use the first option.",
    })),
  },
  new AbortController().signal,
  () => {},
  { hasUI: false, ui: {} },
);
assert.equal(noUiResult.details.cancelled, true);
assert.equal(noUiResult.details.presentation, "tui");
assert.deepEqual(noUiResult.details.questions, []);

const theme = { fg: (_kind, text) => text, bold: (text) => text };
assert.doesNotThrow(() => batchTool.renderResult(noUiResult, {}, theme));

console.log("question smoke tests passed");
