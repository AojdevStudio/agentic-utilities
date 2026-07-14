import assert from "node:assert/strict";
import { createJiti } from "@mariozechner/jiti";

const jiti = createJiti(import.meta.url);
const extensionModule = await jiti.import("./index.ts");
const limits = await jiti.import("./limits.ts");
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

const maximumFieldQuestionnaire = {
  questions: [
    {
      question: "q".repeat(types.MAX_QUESTION_LENGTH),
      header: "h".repeat(types.MAX_HEADER_LENGTH),
      options: [
        {
          label: "a".repeat(types.MAX_LABEL_LENGTH),
          description: "d".repeat(types.MAX_DESCRIPTION_LENGTH),
          preview: "p".repeat(types.MAX_PREVIEW_LENGTH),
        },
        { label: "Second", description: "d".repeat(types.MAX_DESCRIPTION_LENGTH) },
      ],
    },
  ],
};
assert.deepEqual(validator.validateQuestionnaire(maximumFieldQuestionnaire), { ok: true });
assert.equal(
  validator.validateQuestionnaire({
    questions: [{ ...maximumFieldQuestionnaire.questions[0], question: "q".repeat(types.MAX_QUESTION_LENGTH + 1) }],
  }).error,
  "invalid_length",
);
const aggregateTooLarge = {
  questions: Array.from({ length: types.MAX_QUESTIONS }, (_, questionIndex) => ({
    question: `Question ${questionIndex}?`,
    header: `Q${questionIndex}`,
    options: Array.from({ length: types.MAX_OPTIONS }, (_, optionIndex) => ({
      label: `Option ${questionIndex}-${optionIndex}`,
      description: "d".repeat(types.MAX_DESCRIPTION_LENGTH),
      preview: "p".repeat(types.MAX_PREVIEW_LENGTH),
    })),
  })),
};
assert.equal(validator.validateQuestionnaire(aggregateTooLarge).error, "input_too_large");

const maximumCollectionQuestionnaire = {
  questions: Array.from({ length: types.MAX_QUESTIONS }, (_, questionIndex) => ({
    question: `Bounded question ${questionIndex}?`,
    header: `Q${questionIndex}`,
    options: Array.from({ length: types.MAX_OPTIONS }, (_, optionIndex) => ({
      label: `Option ${questionIndex}-${optionIndex}`,
      description: "d".repeat(500),
      preview: "p".repeat(500),
    })),
  })),
};
assert.deepEqual(validator.validateQuestionnaire(maximumCollectionQuestionnaire), { ok: true });
const maximumCollectionEnvelope = response.buildQuestionnaireResponse(
  {
    cancelled: false,
    answers: maximumCollectionQuestionnaire.questions.map((question, questionIndex) => ({
      questionIndex,
      question: question.question,
      kind: "option",
      answer: question.options[0].label,
      preview: question.options[0].preview,
      notes: "n".repeat(types.MAX_NOTES_LENGTH),
    })),
  },
  maximumCollectionQuestionnaire,
);
assert.ok(maximumCollectionEnvelope.content[0].text.length <= limits.QUESTION_LIMITS.toolContent);
assert.ok(JSON.stringify(maximumCollectionEnvelope.details).length <= limits.QUESTION_LIMITS.toolDetails);
assert.equal(maximumCollectionEnvelope.details.cancelled, false);

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
assert.ok(envelope.content[0].text.length <= limits.QUESTION_LIMITS.toolContent);
assert.ok(JSON.stringify(envelope.details).length <= limits.QUESTION_LIMITS.toolDetails);

const oversizedResult = response.buildQuestionnaireResponse(
  {
    cancelled: false,
    answers: Array.from({ length: types.MAX_QUESTIONS }, (_, questionIndex) => ({
      questionIndex,
      question: `Question ${questionIndex}?`,
      kind: "custom",
      answer: "a".repeat(types.MAX_CUSTOM_ANSWER_LENGTH),
      notes: "n".repeat(types.MAX_NOTES_LENGTH),
    })),
  },
  {
    questions: Array.from({ length: types.MAX_QUESTIONS }, (_, questionIndex) => ({
      question: `Question ${questionIndex}?`,
      header: `Q${questionIndex}`,
      options: [
        { label: "One", description: "First." },
        { label: "Two", description: "Second." },
      ],
    })),
  },
);
assert.equal(oversizedResult.details.error, "result_too_large");
assert.equal(oversizedResult.details.answers.length, 0);
assert.ok(oversizedResult.content[0].text.length <= limits.QUESTION_LIMITS.toolContent);

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
  ["agentic_utilities_ask_user_question", "AskUserQuestion", "AskBatchQuestions"],
);

const singleTool = tools.find((tool) => tool.name === "AskUserQuestion");
const oversizedSingle = await singleTool.execute(
  "tool-call-id",
  { question: "q".repeat(limits.QUESTION_LIMITS.questionText + 1), type: "text" },
  new AbortController().signal,
  () => {},
  { hasUI: false, ui: {} },
);
assert.equal(oversizedSingle.details.cancelled, true);
assert.match(oversizedSingle.content[0].text, /field or collection limit/);

const oversizedBrowserAnswer = {
  cancelled: false,
  answers: [
    {
      question: "Bounded?",
      questionType: "text",
      answer: "a".repeat(limits.QUESTION_LIMITS.customAnswer + 1),
      value: "a",
      selectedLabel: null,
      selectedLabels: null,
      selectedValues: null,
      isCustomInput: true,
    },
  ],
};
assert.equal(extensionModule.parseBatchSubmission(JSON.stringify(oversizedBrowserAnswer)).status, 400);
const aggregateBrowserAnswer = {
  cancelled: false,
  answers: Array.from({ length: 3 }, (_, index) => ({
    question: `Question ${index}?`,
    questionType: "text",
    answer: "a".repeat(limits.QUESTION_LIMITS.customAnswer),
    value: "a".repeat(limits.QUESTION_LIMITS.customAnswer),
    selectedLabel: null,
    selectedLabels: null,
    selectedValues: null,
    isCustomInput: true,
  })),
};
assert.equal(extensionModule.parseBatchSubmission(JSON.stringify(aggregateBrowserAnswer)).status, 413);

const batchTool = tools.find((tool) => tool.name === "AskBatchQuestions");
const noUiResult = await batchTool.execute(
  "tool-call-id",
  {
    title: "TUI no UI smoke",
    presentation: "tui",
    questions: valid.questions.map((question) => ({
      question: question.question,
      type: "decision",
      options: question.options.map((option) => ({
        ...option,
        preview: option.preview ? { kind: "markdown", content: option.preview } : undefined,
      })),
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
