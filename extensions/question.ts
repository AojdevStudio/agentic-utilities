// @ts-nocheck

import { execSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { unlinkSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { pathToFileURL } from "node:url";
import { StringEnum } from "@mariozechner/pi-ai";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { Text } from "@mariozechner/pi-tui";
import { Type } from "typebox";
import { Value } from "typebox/value";
import {
  executeAskUserQuestionnaire,
  registerAskUserQuestionTool as registerRpivAskUserQuestionTool,
} from "./question/rpiv/ask-user-question.js";

type QuestionType = "decision" | "single-choice" | "multi-select" | "text";
type Presentation = "auto" | "tui" | "browser";

type PrimitiveAnswer = string | string[] | null;

type BatchSubmitPayload = {
  cancelled: boolean;
  answers: BatchQuestionAnswer[];
};

interface QuestionUi {
  input(prompt: string, placeholder?: string): Promise<string | undefined>;
  select(prompt: string, options: string[]): Promise<string | undefined>;
  notify(message: string, level?: "info" | "success" | "warning" | "error"): void;
}

interface QuestionExecutionContext {
  hasUI: boolean;
  ui: QuestionUi;
}

interface QuestionTheme {
  fg(kind: string, text: string): string;
  bold(text: string): string;
}

const QuestionTypeSchema = StringEnum(["decision", "single-choice", "multi-select", "text"] as const);
const PresentationSchema = StringEnum(["auto", "tui", "browser"] as const);
const PreviewKindSchema = StringEnum(["text", "markdown", "code", "mermaid", "image", "url"] as const);
const NoteToneSchema = StringEnum(["info", "warning", "success", "danger"] as const);

const PreviewSchema = Type.Object({
  kind: PreviewKindSchema,
  title: Type.Optional(Type.String({ description: "Optional preview title" })),
  content: Type.Optional(Type.String({ description: "Preview content shown beside the options" })),
  language: Type.Optional(Type.String({ description: "Language for code previews" })),
  path: Type.Optional(Type.String({ description: "Optional local/example path referenced by the preview" })),
  url: Type.Optional(Type.String({ description: "Optional URL referenced by the preview" })),
  collapsed: Type.Optional(Type.Boolean({ description: "Whether to start the preview collapsed" })),
  i18nKey: Type.Optional(Type.String({ description: "Optional localization key for preview text" })),
});

const NoteSchema = Type.Object({
  title: Type.Optional(Type.String({ description: "Optional note title" })),
  body: Type.String({ description: "Short context, warning, or trade-off note" }),
  tone: Type.Optional(NoteToneSchema),
  i18nKey: Type.Optional(Type.String({ description: "Optional localization key for note text" })),
});

const I18nSchema = Type.Object({
  locale: Type.Optional(Type.String({ description: "Optional locale code" })),
  namespace: Type.Optional(Type.String({ description: "Optional localization namespace" })),
  strings: Type.Optional(Type.Record(Type.String(), Type.String(), { description: "Inline localization strings" })),
  keys: Type.Optional(Type.Record(Type.String(), Type.String(), { description: "Localization key mapping" })),
});

const OptionSchema = Type.Object({
  label: Type.String({ description: "Display label shown to the user" }),
  value: Type.Optional(Type.String({ description: "Optional stable value returned for this option" })),
  description: Type.Optional(Type.String({ description: "Optional secondary description" })),
  preview: Type.Optional(PreviewSchema),
  notes: Type.Optional(Type.Array(NoteSchema, { description: "Optional notes or trade-offs for this option" })),
  i18nKey: Type.Optional(Type.String({ description: "Optional localization key for this option" })),
});

const AskUserQuestionParams = Type.Object({
  question: Type.String({ description: "The question to ask the user" }),
  type: Type.Optional(QuestionTypeSchema, {
    description:
      "Question type. Use 'single-choice' for one option, 'multi-select' for multiple options, or 'text' for fill-in-the-blank input. Defaults to 'text' when no options are provided, otherwise 'single-choice'.",
  }),
  options: Type.Optional(
    Type.Array(OptionSchema, {
      description: "Optional choices. If omitted, the user gets a free-form input box.",
    }),
  ),
  allowOther: Type.Optional(
    Type.Boolean({
      description:
        "When options are provided, also allow a free-form 'Type something...' answer (default: true). For multi-select, custom text can be combined with selected options.",
    }),
  ),
  placeholder: Type.Optional(Type.String({ description: "Placeholder text for free-form input" })),
  recommendedOption: Type.Optional(Type.String({ description: "Stable value or label for the recommended option" })),
  preview: Type.Optional(PreviewSchema),
  notes: Type.Optional(Type.Array(NoteSchema, { description: "Optional notes or trade-offs for this question" })),
  allowUserNote: Type.Optional(
    Type.Boolean({ description: "Allow the user to attach a note to their answer when supported" }),
  ),
  i18nKey: Type.Optional(Type.String({ description: "Optional localization key for this question" })),
  i18n: Type.Optional(I18nSchema),
  mermaid: Type.Optional(
    Type.String({
      description:
        "Optional Mermaid diagram source to help the user understand relationships, flows, or architecture before answering.",
    }),
  ),
  recommendation: Type.Optional(
    Type.String({
      description: "Agent's recommendation for this question based on scope and context.",
    }),
  ),
});

const QuestionItemSchema = Type.Object({
  question: Type.String({ description: "The question to ask the user" }),
  type: Type.Optional(QuestionTypeSchema, {
    description:
      "Question type. Use 'single-choice' for one option, 'multi-select' for multiple options, or 'text' for fill-in-the-blank input. Defaults to 'text' when no options are provided, otherwise 'single-choice'.",
  }),
  options: Type.Optional(
    Type.Array(OptionSchema, {
      description: "Optional choices. If omitted, the user gets a free-form input box.",
    }),
  ),
  allowOther: Type.Optional(
    Type.Boolean({
      description:
        "When options are provided, also allow a free-form custom answer. For multi-select, custom text can be combined with selected options.",
    }),
  ),
  placeholder: Type.Optional(Type.String({ description: "Placeholder text for free-form input" })),
  recommendedOption: Type.Optional(Type.String({ description: "Stable value or label for the recommended option" })),
  preview: Type.Optional(PreviewSchema),
  notes: Type.Optional(Type.Array(NoteSchema, { description: "Optional notes or trade-offs for this question" })),
  allowUserNote: Type.Optional(
    Type.Boolean({ description: "Allow the user to attach a note to their answer when supported" }),
  ),
  i18nKey: Type.Optional(Type.String({ description: "Optional localization key for this question" })),
  recommendation: Type.Optional(
    Type.String({
      description: "Agent's recommendation for this question based on scope and context.",
    }),
  ),
});

const AskBatchQuestionsParams = Type.Object({
  title: Type.String({ description: "Title for the questionnaire" }),
  description: Type.Optional(Type.String({ description: "Description or context for the questionnaire" })),
  questions: Type.Array(QuestionItemSchema, {
    description: "Array of questions to ask the user",
  }),
  presentation: Type.Optional(PresentationSchema),
  preview: Type.Optional(PreviewSchema),
  notes: Type.Optional(Type.Array(NoteSchema, { description: "Optional notes for the whole questionnaire" })),
  i18n: Type.Optional(I18nSchema),
});

interface MermaidVisualDetails {
  source: string;
  ascii: string;
  lineCount: number;
  index: number;
}

interface AskUserQuestionDetails {
  question: string;
  questionType: QuestionType;
  options: string[];
  answer: PrimitiveAnswer;
  value: PrimitiveAnswer;
  selectedLabels?: string[] | null;
  selectedValues?: string[] | null;
  cancelled: boolean;
  mode: "decision" | "select" | "input" | "other" | "multi-select" | "unavailable";
  mermaid?: MermaidVisualDetails;
  visualError?: string | null;
  recommendation?: string | null;
}

interface BatchQuestionAnswer {
  question: string;
  questionType: QuestionType;
  answer: PrimitiveAnswer;
  value: PrimitiveAnswer;
  selectedLabel: string | null;
  selectedLabels: string[] | null;
  selectedValues: string[] | null;
  isCustomInput: boolean;
}

interface AskBatchQuestionsDetails {
  title: string;
  description: string | null;
  questions: BatchQuestionAnswer[];
  cancelled: boolean;
  presentation?: Presentation;
}

interface QuestionPreview {
  kind: "text" | "markdown" | "code" | "mermaid" | "image" | "url";
  title?: string;
  content?: string;
  language?: string;
  path?: string;
  url?: string;
  collapsed?: boolean;
  i18nKey?: string;
}

interface QuestionNote {
  title?: string;
  body: string;
  tone?: "info" | "warning" | "success" | "danger";
  i18nKey?: string;
}

interface QuestionOption {
  label: string;
  value?: string;
  description?: string;
  preview?: QuestionPreview | string;
  notes?: QuestionNote[];
  i18nKey?: string;
}

interface QuestionItem {
  id?: string;
  question: string;
  type?: QuestionType;
  options?: QuestionOption[];
  allowOther?: boolean;
  placeholder?: string;
  recommendation?: string;
  recommendedOption?: string;
  preview?: QuestionPreview;
  notes?: QuestionNote[];
  allowUserNote?: boolean;
  i18nKey?: string;
}

const PrimitiveAnswerSchema = Type.Union([Type.String(), Type.Array(Type.String()), Type.Null()]);

const BatchQuestionAnswerSchema = Type.Object(
  {
    question: Type.String(),
    questionType: QuestionTypeSchema,
    answer: PrimitiveAnswerSchema,
    value: PrimitiveAnswerSchema,
    selectedLabel: Type.Union([Type.String(), Type.Null()]),
    selectedLabels: Type.Union([Type.Array(Type.String()), Type.Null()]),
    selectedValues: Type.Union([Type.Array(Type.String()), Type.Null()]),
    isCustomInput: Type.Boolean(),
  },
  { additionalProperties: false },
);

const BatchSubmitPayloadSchema = Type.Object(
  {
    cancelled: Type.Boolean(),
    answers: Type.Array(BatchQuestionAnswerSchema),
  },
  { additionalProperties: false },
);

const OTHER_LABEL = "Type something...";
const DONE_LABEL = "Submit selection";
const MAX_PROMPT_DIAGRAM_LINES = 18;
const MAX_PROMPT_DIAGRAM_CHARS = 4000;
const MAX_RESULT_BODY_BYTES = 1024 * 1024;
const QUESTIONNAIRE_TIMEOUT_MS = 300000;

let mermaidRendererPromise: Promise<((source: string) => string) | null> | null = null;
let globalNpmRoot: string | null | undefined;

function countLines(text: string): number {
  if (!text) return 0;
  return text.split(/\r?\n/).length;
}

function normalizeQuestionType(type: QuestionType | undefined, options: QuestionOption[] | undefined): QuestionType {
  if (!options || options.length === 0) return "text";
  return type ?? "single-choice";
}

function normalizeQuestionItem(
  question: QuestionItem,
): QuestionItem & { type: QuestionType; options: QuestionOption[] } {
  const options = question.options ?? [];
  const type = normalizeQuestionType(question.type, options);
  return {
    ...question,
    options,
    type,
    allowOther: type === "decision" ? false : question.allowOther,
  };
}

function formatAnswer(answer: PrimitiveAnswer): string {
  if (Array.isArray(answer)) return answer.join(", ");
  return answer ?? "(no answer)";
}

async function readTrimmedInput(
  ctx: QuestionExecutionContext,
  prompt: string,
  placeholder?: string,
): Promise<string | undefined> {
  while (true) {
    const answer = await ctx.ui.input(prompt, placeholder ?? "Type your answer");
    if (answer === undefined) return undefined;
    const trimmed = answer.trim();
    if (trimmed) return trimmed;
    ctx.ui.notify("Answer cannot be blank.", "warning");
  }
}

function getGlobalNpmRoot(): string | null {
  if (globalNpmRoot !== undefined) return globalNpmRoot;

  try {
    globalNpmRoot = execSync("npm root -g", { encoding: "utf8" }).trim() || null;
  } catch {
    globalNpmRoot = null;
  }

  return globalNpmRoot;
}

async function getMermaidAsciiRenderer(): Promise<((source: string) => string) | null> {
  if (mermaidRendererPromise) return mermaidRendererPromise;

  mermaidRendererPromise = (async () => {
    const candidates = ["beautiful-mermaid"];
    const npmRoot = getGlobalNpmRoot();
    if (npmRoot) {
      candidates.push(join(npmRoot, "pi-mermaid", "node_modules", "beautiful-mermaid", "dist", "index.js"));
      candidates.push(join(npmRoot, "beautiful-mermaid", "dist", "index.js"));
    }

    for (const candidate of candidates) {
      try {
        const mod = isAbsolute(candidate) ? await import(pathToFileURL(candidate).href) : await import(candidate);
        const renderMermaidAscii = (mod as { renderMermaidAscii?: unknown }).renderMermaidAscii;
        if (typeof renderMermaidAscii === "function") {
          return (source: string) =>
            (renderMermaidAscii as (text: string, options?: Record<string, unknown>) => string)(source, {
              paddingX: 2,
              boxBorderPadding: 0,
              colorMode: "none",
            }).trimEnd();
        }
      } catch {
        // try next candidate
      }
    }

    return null;
  })();

  return mermaidRendererPromise;
}

async function buildMermaidVisual(
  source: string | undefined,
): Promise<{ visual?: MermaidVisualDetails; error?: string }> {
  const mermaid = source?.trim();
  if (!mermaid) return {};

  const renderer = await getMermaidAsciiRenderer();
  if (!renderer) {
    return {
      error:
        "Mermaid renderer unavailable. pi-mermaid is installed, but beautiful-mermaid could not be loaded from the current runtime.",
    };
  }

  try {
    const ascii = renderer(mermaid);
    return {
      visual: {
        source: mermaid,
        ascii,
        lineCount: countLines(ascii),
        index: 1,
      },
    };
  } catch (error) {
    return {
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function getPromptDiagram(ascii: string | undefined): string | undefined {
  const text = ascii?.trimEnd();
  if (!text) return undefined;

  let truncated = false;
  let lines = text.split(/\r?\n/);
  if (lines.length > MAX_PROMPT_DIAGRAM_LINES) {
    lines = lines.slice(0, MAX_PROMPT_DIAGRAM_LINES);
    truncated = true;
  }

  let preview = lines.join("\n");
  if (preview.length > MAX_PROMPT_DIAGRAM_CHARS) {
    preview = preview.slice(0, MAX_PROMPT_DIAGRAM_CHARS).trimEnd();
    truncated = true;
  }

  if (truncated) preview += "\n...";
  return preview;
}

function buildPrompt(
  question: string,
  visual?: MermaidVisualDetails,
  visualError?: string,
  options: QuestionOption[] = [],
  recommendation?: string,
): string {
  const lines = [question];

  if (recommendation?.trim()) {
    lines.push("", `Recommendation: ${recommendation.trim()}`);
  }

  if (options.some((option) => option.description?.trim())) {
    lines.push("", "Options:");
    for (const option of options) {
      lines.push(option.description?.trim() ? `- ${option.label}: ${option.description.trim()}` : `- ${option.label}`);
    }
  }

  const diagram = getPromptDiagram(visual?.ascii);
  if (diagram) lines.push("", "Mermaid (ASCII)", diagram);
  else if (visualError) lines.push("", `[Visual unavailable: ${visualError}]`);

  return lines.join("\n");
}

function hasPiMermaid(pi: ExtensionAPI): boolean {
  return pi.getCommands().some((command) => command.name === "pi-mermaid");
}

function emitPiMermaidMessage(pi: ExtensionAPI, visual: MermaidVisualDetails): void {
  if (!hasPiMermaid(pi)) return;

  pi.sendMessage({
    customType: "pi-mermaid",
    content: `\`\`\`mermaid\n${visual.source}\n\`\`\``,
    display: true,
    details: visual,
  });
}

function buildResultText(
  prefix: string,
  details: AskUserQuestionDetails,
  expanded: boolean,
  theme: QuestionTheme,
): string {
  let text = prefix;
  if (details.mermaid?.ascii) {
    text += "\n" + theme.fg("muted", "Visual included");
    if (expanded) text += "\n\n" + details.mermaid.ascii;
  } else if (details.visualError) {
    text += "\n" + theme.fg("warning", `Visual unavailable: ${details.visualError}`);
  }
  if (details.recommendation) {
    text += "\n" + theme.fg("info", `💡 Recommendation: ${details.recommendation}`);
  }
  return text;
}

async function askMultiSelectInTui(
  ctx: QuestionExecutionContext,
  prompt: string,
  options: QuestionOption[],
  allowOther: boolean,
  placeholder?: string,
): Promise<{ cancelled: boolean; labels: string[]; values: string[] }> {
  if (options.length === 0 && !allowOther) {
    ctx.ui.notify("No options available for this multi-select question.", "warning");
    return { cancelled: true, labels: [], values: [] };
  }

  const selected = new Set<number>();
  let customAnswer: string | null = null;

  while (true) {
    const labels = options.map((option, index) => `${selected.has(index) ? "[x]" : "[ ]"} ${option.label}`);
    const menu = [...labels];
    if (allowOther) menu.push(customAnswer ? `Custom: ${customAnswer}` : OTHER_LABEL);
    menu.push(DONE_LABEL);

    const selection = await ctx.ui.select(prompt, menu);
    if (selection === undefined) {
      return { cancelled: true, labels: [], values: [] };
    }

    if (selection === DONE_LABEL) {
      const picked = [...selected].map((index) => options[index]);
      const answerLabels = picked.map((option) => option.label);
      const answerValues = picked.map((option) => option.value ?? option.label);
      if (customAnswer) {
        answerLabels.push(customAnswer);
        answerValues.push(customAnswer);
      }
      if (answerLabels.length === 0) {
        ctx.ui.notify("Select at least one option or type a custom answer.", "warning");
        continue;
      }
      return { cancelled: false, labels: answerLabels, values: answerValues };
    }

    if (allowOther && (selection === OTHER_LABEL || selection.startsWith("Custom: "))) {
      const answer = await readTrimmedInput(ctx, prompt, placeholder);
      if (answer === undefined) {
        continue;
      }
      customAnswer = answer;
      continue;
    }

    const optionIndex = labels.indexOf(selection);
    if (optionIndex >= 0) {
      if (selected.has(optionIndex)) selected.delete(optionIndex);
      else selected.add(optionIndex);
    }
  }
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function escapeJsonForScriptTag(json: string): string {
  return json
    .replace(/</g, "\\u003c")
    .replace(/>/g, "\\u003e")
    .replace(/&/g, "\\u0026")
    .replace(/\u2028/g, "\\u2028")
    .replace(/\u2029/g, "\\u2029");
}

export function generateBatchQuestionsHTML(params: {
  title: string;
  description?: string;
  questions: QuestionItem[];
  serverPort: number;
  sessionToken: string;
  expiresAt: number;
}): string {
  const normalizedQuestions = params.questions.map(normalizeQuestionItem);

  const { title, description, serverPort, sessionToken, expiresAt } = params;
  const questionsJson = escapeJsonForScriptTag(JSON.stringify(normalizedQuestions));
  const sessionTokenJson = escapeJsonForScriptTag(JSON.stringify(sessionToken));
  const expiresAtJson = escapeJsonForScriptTag(JSON.stringify(expiresAt));

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(title)}</title>
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 24px;
      background: #f5f5f5;
      color: #1a1a1a;
      font: 14px/1.5 -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    }
    .container {
      max-width: 860px;
      margin: 0 auto;
      background: #fff;
      border: 1px solid #d8d8d8;
    }
    .header, .progress, .actions {
      padding: 18px 24px;
      border-bottom: 1px solid #e8e8e8;
      background: #fff;
    }
    .actions {
      border-top: 1px solid #e8e8e8;
      border-bottom: 0;
      display: flex;
      justify-content: flex-end;
      gap: 10px;
      background: #fafafa;
    }
    h1 {
      margin: 0 0 4px 0;
      font-size: 18px;
    }
    .header p, .progress-text {
      margin: 0;
      color: #666;
      font-size: 13px;
    }
    .progress { position: sticky; top: 0; z-index: 2; }
    .progress-bar {
      width: 100%;
      height: 4px;
      background: #ececec;
      margin-bottom: 8px;
    }
    .progress-fill {
      height: 100%;
      width: 0%;
      background: #1a1a1a;
      transition: width 0.2s ease;
    }
    .content { padding: 24px; }
    .status-banner {
      margin: 0 24px;
      padding: 12px 16px;
      border: 1px solid #e8d9a1;
      background: #fff9e8;
      color: #6b5a1f;
      font-size: 13px;
      display: none;
    }
    .status-banner.visible {
      display: block;
    }
    .question-card {
      border: 1px solid #d8d8d8;
      padding: 18px;
      margin-bottom: 16px;
      background: #fff;
    }
    .question-card.answered { border-color: #1a1a1a; }
    .question-head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 12px;
      margin-bottom: 12px;
    }
    .question-text { font-weight: 600; }
    .meta { display: flex; gap: 8px; flex-wrap: wrap; }
    .badge {
      padding: 2px 8px;
      border: 1px solid #d8d8d8;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.4px;
      color: #666;
      white-space: nowrap;
    }
    .badge.answered { border-color: #1a1a1a; color: #1a1a1a; }
    .recommendation {
      margin-bottom: 12px;
      padding: 10px 12px;
      background: #fafafa;
      border: 1px solid #e8e8e8;
      border-left: 3px solid #666;
      font-size: 13px;
      color: #444;
    }
    .recommendation strong { color: #1a1a1a; }
    .options { display: grid; gap: 8px; }
    .option-btn, .multi-option {
      border: 1px solid #d8d8d8;
      background: #fff;
      padding: 10px 12px;
      width: 100%;
      text-align: left;
      font: inherit;
      color: inherit;
    }
    .option-btn { cursor: pointer; }
    .option-btn:hover, .multi-option:hover {
      border-color: #1a1a1a;
      background: #fafafa;
    }
    .option-btn.selected, .multi-option.selected {
      border-color: #1a1a1a;
      background: #f1f1f1;
    }
    .multi-option {
      display: flex;
      align-items: flex-start;
      gap: 10px;
      cursor: pointer;
    }
    .multi-option input { margin-top: 3px; }
    .option-main { flex: 1; }
    .option-description {
      margin-top: 4px;
      font-size: 12px;
      color: #666;
    }
    textarea {
      width: 100%;
      min-height: 84px;
      padding: 10px 12px;
      border: 1px solid #d8d8d8;
      font: inherit;
      resize: vertical;
      color: inherit;
      background: #fff;
    }
    textarea:focus { outline: none; border-color: #1a1a1a; }
    .custom-input { margin-top: 10px; }
    .help-text {
      margin-top: 8px;
      font-size: 12px;
      color: #666;
    }
    .btn {
      padding: 8px 16px;
      border: 1px solid #d8d8d8;
      background: #fff;
      font: inherit;
      cursor: pointer;
    }
    .btn:hover { background: #f0f0f0; }
    .btn-submit {
      background: #1a1a1a;
      color: #fff;
      border-color: #1a1a1a;
    }
    .btn-submit:hover { background: #000; }
    .btn-submit:disabled {
      background: #b7b7b7;
      border-color: #b7b7b7;
      cursor: not-allowed;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>${escapeHtml(title)}</h1>
      ${description ? `<p>${escapeHtml(description)}</p>` : ""}
    </div>
    <div class="progress">
      <div class="progress-bar"><div class="progress-fill" id="progressFill"></div></div>
      <p class="progress-text" id="progressText">0 of ${normalizedQuestions.length} questions answered</p>
    </div>
    <div class="status-banner" id="statusBanner"></div>
    <div class="content">
      ${normalizedQuestions
        .map((q, index) => {
          const questionType = q.type === "decision" ? "single-choice" : (q.type ?? "single-choice");
          return `
          <section class="question-card" id="question-${index}" data-question="${index}" data-type="${escapeHtml(questionType)}">
            <div class="question-head">
              <div class="question-text">${escapeHtml(q.question)}</div>
              <div class="meta">
                <span class="badge">${escapeHtml(questionType)}</span>
                <span class="badge" id="badge-${index}">Pending</span>
              </div>
            </div>
            ${q.recommendation ? `<div class="recommendation"><strong>Recommended:</strong> ${escapeHtml(q.recommendation)}</div>` : ""}
            ${
              questionType === "single-choice" && q.options?.length
                ? `
              <div class="options single-options">
                ${q.options
                  .map(
                    (option, optionIndex) => `
                  <button type="button" class="option-btn" data-question="${index}" data-option="${optionIndex}" data-label="${escapeHtml(option.label)}" data-value="${escapeHtml(option.value || option.label)}">
                    <div class="option-main">
                      <div>${escapeHtml(option.label)}</div>
                      ${option.description ? `<div class="option-description">${escapeHtml(option.description)}</div>` : ""}
                    </div>
                  </button>
                `,
                  )
                  .join("")}
              </div>
            `
                : ""
            }
            ${
              questionType === "multi-select" && q.options?.length
                ? `
              <div class="options multi-options">
                ${q.options
                  .map(
                    (option, optionIndex) => `
                  <label class="multi-option" data-question="${index}" data-option="${optionIndex}">
                    <input type="checkbox" data-question="${index}" data-option="${optionIndex}" data-label="${escapeHtml(option.label)}" data-value="${escapeHtml(option.value || option.label)}">
                    <div class="option-main">
                      <div>${escapeHtml(option.label)}</div>
                      ${option.description ? `<div class="option-description">${escapeHtml(option.description)}</div>` : ""}
                    </div>
                  </label>
                `,
                  )
                  .join("")}
              </div>
            `
                : ""
            }
            ${
              questionType === "text" || q.allowOther !== false || !q.options?.length
                ? `
              <div class="custom-input">
                <textarea id="textarea-${index}" placeholder="${escapeHtml(q.placeholder || "Type your answer here...")}"></textarea>
                <div class="help-text">${questionType === "multi-select" ? "Optional: add custom text in addition to checked options." : questionType === "single-choice" ? "Optional: type a custom answer instead of selecting an option." : "Fill in your answer."}</div>
              </div>
            `
                : ""
            }
          </section>
        `;
        })
        .join("")}
    </div>
    <div class="actions">
      <button class="btn" id="cancelBtn" type="button">Cancel</button>
      <button class="btn btn-submit" id="submitBtn" type="button" disabled>Submit Answers</button>
    </div>
  </div>
  <script type="application/json" id="pi-questionnaire-data">${questionsJson}</script>
  <script type="application/json" id="pi-questionnaire-token">${sessionTokenJson}</script>
  <script type="application/json" id="pi-questionnaire-expires-at">${expiresAtJson}</script>
  <script>
    const questions = JSON.parse(document.getElementById('pi-questionnaire-data').textContent || '[]');
    const sessionToken = JSON.parse(document.getElementById('pi-questionnaire-token').textContent || '""');
    const expiresAt = JSON.parse(document.getElementById('pi-questionnaire-expires-at').textContent || '0');
    const serverPort = ${serverPort};
    const state = Object.fromEntries(questions.map((q, i) => [i, { selectedOptions: [], customInput: "" }]));
    let sessionExpired = false;

    function showStatus(message) {
      const banner = document.getElementById('statusBanner');
      if (!banner) return;
      banner.textContent = message;
      banner.classList.add('visible');
    }

    function expireSession(message) {
      if (sessionExpired) return;
      sessionExpired = true;
      showStatus(message || 'This questionnaire session expired. Please rerun it from the terminal.');
      const submitBtn = document.getElementById('submitBtn');
      const cancelBtn = document.getElementById('cancelBtn');
      if (submitBtn) {
        submitBtn.disabled = true;
        submitBtn.textContent = 'Session Expired';
      }
      if (cancelBtn) cancelBtn.disabled = true;
    }

    function isSessionExpired() {
      return sessionExpired || (expiresAt > 0 && Date.now() >= expiresAt);
    }

    function isAnswered(questionIndex) {
      const q = questions[questionIndex];
      const s = state[questionIndex];
      const hasCustom = !!(s.customInput && s.customInput.trim());
      const selectedCount = s.selectedOptions.length;
      if (q.type === 'text') return hasCustom;
      if (q.type === 'multi-select') return selectedCount > 0 || hasCustom;
      return selectedCount === 1 || hasCustom;
    }

    function updateQuestionCard(questionIndex) {
      const card = document.getElementById('question-' + questionIndex);
      const badge = document.getElementById('badge-' + questionIndex);
      if (isAnswered(questionIndex)) {
        card.classList.add('answered');
        badge.classList.add('answered');
        badge.textContent = 'Answered';
      } else {
        card.classList.remove('answered');
        badge.classList.remove('answered');
        badge.textContent = 'Pending';
      }
    }

    function updateProgress() {
      const answered = questions.filter((_, index) => isAnswered(index)).length;
      const percentage = questions.length === 0 ? 100 : (answered / questions.length) * 100;
      document.getElementById('progressFill').style.width = percentage + '%';
      document.getElementById('progressText').textContent = answered + ' of ' + questions.length + ' questions answered';
      const submitBtn = document.getElementById('submitBtn');
      if (!submitBtn) return;
      if (isSessionExpired()) {
        expireSession('This questionnaire session expired. Please rerun it from the terminal.');
        return;
      }
      submitBtn.disabled = answered < questions.length;
    }

    function setSingleChoice(questionIndex, label, value, buttonEl) {
      state[questionIndex].selectedOptions = [{ label, value }];
      state[questionIndex].customInput = '';
      document.querySelectorAll('.option-btn[data-question="' + questionIndex + '"]').forEach((btn) => btn.classList.remove('selected'));
      buttonEl.classList.add('selected');
      const textarea = document.getElementById('textarea-' + questionIndex);
      if (textarea) textarea.value = '';
      updateQuestionCard(questionIndex);
      updateProgress();
    }

    function syncMultiChoice(questionIndex) {
      const selected = [];
      document.querySelectorAll('input[type="checkbox"][data-question="' + questionIndex + '"]').forEach((input) => {
        const wrapper = input.closest('.multi-option');
        if (input.checked) {
          selected.push({ label: input.dataset.label, value: input.dataset.value });
          if (wrapper) wrapper.classList.add('selected');
        } else if (wrapper) {
          wrapper.classList.remove('selected');
        }
      });
      state[questionIndex].selectedOptions = selected;
      updateQuestionCard(questionIndex);
      updateProgress();
    }

    document.querySelectorAll('.option-btn').forEach((button) => {
      button.addEventListener('click', function () {
        setSingleChoice(Number(this.dataset.question), this.dataset.label, this.dataset.value, this);
      });
    });

    document.querySelectorAll('input[type="checkbox"]').forEach((input) => {
      input.addEventListener('change', function () {
        syncMultiChoice(Number(this.dataset.question));
      });
    });

    document.querySelectorAll('textarea').forEach((textarea) => {
      textarea.addEventListener('input', function () {
        const questionIndex = Number(this.id.replace('textarea-', ''));
        const question = questions[questionIndex];
        state[questionIndex].customInput = this.value;

        if (question.type === 'single-choice' && this.value.trim()) {
          state[questionIndex].selectedOptions = [];
          document.querySelectorAll('.option-btn[data-question="' + questionIndex + '"]').forEach((btn) => btn.classList.remove('selected'));
        }

        updateQuestionCard(questionIndex);
        updateProgress();
      });
    });

    function buildAnswer(questionIndex) {
      const question = questions[questionIndex];
      const stateItem = state[questionIndex];
      const customInput = stateItem.customInput.trim();
      const selectedLabels = stateItem.selectedOptions.map((option) => option.label);
      const selectedValues = stateItem.selectedOptions.map((option) => option.value);

      if (question.type === 'multi-select') {
        const answer = customInput ? [...selectedLabels, customInput] : selectedLabels;
        const value = customInput ? [...selectedValues, customInput] : selectedValues;
        return {
          question: question.question,
          questionType: question.type,
          answer,
          value,
          selectedLabel: null,
          selectedLabels,
          selectedValues,
          isCustomInput: !!customInput,
        };
      }

      if (question.type === 'text') {
        return {
          question: question.question,
          questionType: question.type,
          answer: customInput || null,
          value: customInput || null,
          selectedLabel: null,
          selectedLabels: null,
          selectedValues: null,
          isCustomInput: !!customInput,
        };
      }

      if (customInput) {
        return {
          question: question.question,
          questionType: question.type,
          answer: customInput,
          value: customInput,
          selectedLabel: null,
          selectedLabels: null,
          selectedValues: null,
          isCustomInput: true,
        };
      }

      const selected = stateItem.selectedOptions[0] || null;
      return {
        question: question.question,
        questionType: question.type,
        answer: selected ? selected.label : null,
        value: selected ? selected.value : null,
        selectedLabel: selected ? selected.label : null,
        selectedLabels: selected ? [selected.label] : null,
        selectedValues: selected ? [selected.value] : null,
        isCustomInput: false,
      };
    }

    async function submitPayload(payload) {
      const response = await fetch('http://127.0.0.1:' + serverPort + '/submit', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Pi-Question-Token': sessionToken,
        },
        body: JSON.stringify(payload),
      });
      if (!response.ok) throw new Error('Server returned ' + response.status);
    }

    document.getElementById('submitBtn').addEventListener('click', async function () {
      if (isSessionExpired()) {
        expireSession('This questionnaire session expired before submission. Please rerun it from the terminal.');
        return;
      }

      this.disabled = true;
      this.textContent = 'Submitting...';

      const payload = {
        cancelled: false,
        answers: questions.map((_, index) => buildAnswer(index)),
      };

      try {
        await submitPayload(payload);
        document.body.innerHTML = '<div class="container"><div class="header"><h1>✓ Answers Submitted</h1><p>You can close this tab now.</p></div></div>';
      } catch (error) {
        if (error instanceof Error && error.message.includes('403')) {
          expireSession('This questionnaire session expired before the server accepted your answers. Please rerun it from the terminal.');
          return;
        }
        const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'pi-questions-result.json';
        a.click();
        URL.revokeObjectURL(url);
        document.body.innerHTML = '<div class="container"><div class="header"><h1>Submission Failed</h1><p>Could not reach local server. Answers were saved locally as JSON for recovery.</p></div></div>';
      }
    });

    document.getElementById('cancelBtn').addEventListener('click', async function () {
      if (isSessionExpired()) {
        expireSession('This questionnaire session expired. Please rerun it from the terminal.');
        return;
      }
      if (!confirm('Cancel this questionnaire? Your answers will be lost.')) return;
      try {
        await submitPayload({ cancelled: true, answers: [] });
      } catch {}
      document.body.innerHTML = '<div class="container"><div class="header"><h1>Cancelled</h1><p>You can close this tab now.</p></div></div>';
    });

    const msUntilExpiry = expiresAt > 0 ? expiresAt - Date.now() : 0;
    if (msUntilExpiry <= 0) {
      expireSession('This questionnaire session expired. Please rerun it from the terminal.');
    } else {
      setTimeout(() => {
        expireSession('This questionnaire session expired. Please rerun it from the terminal.');
      }, msUntilExpiry);
    }

    updateProgress();
    questions.forEach((_, index) => updateQuestionCard(index));
  </script>
</body>
</html>`;
}

function parseBatchSubmission(
  body: string,
): { ok: true; value: BatchSubmitPayload } | { ok: false; status: number; error: string } {
  try {
    const parsed: unknown = JSON.parse(body);
    if (!Value.Check(BatchSubmitPayloadSchema, parsed)) {
      return { ok: false, status: 400, error: "Invalid submission payload" };
    }
    return { ok: true, value: parsed as BatchSubmitPayload };
  } catch {
    return { ok: false, status: 400, error: "Invalid JSON" };
  }
}

function startResultServer(
  sessionToken: string,
): Promise<{ server: Server; port: number; resultPromise: Promise<BatchSubmitPayload> }> {
  let resolveResult: (result: BatchSubmitPayload) => void = () => {};
  let settled = false;
  const resultPromise = new Promise<BatchSubmitPayload>((resolve) => {
    resolveResult = resolve;
  });

  const server = createServer((req, res) => {
    const sendJson = (statusCode: number, payload: unknown): void => {
      if (res.writableEnded) return;
      res.writeHead(statusCode, { "Content-Type": "application/json" });
      res.end(JSON.stringify(payload));
    };

    const origin = req.headers.origin;
    const isAllowedOrigin =
      typeof origin === "string" && /^(null|file:\/\/|https?:\/\/(127\.0\.0\.1|localhost)(:\d+)?)$/.test(origin);

    if (isAllowedOrigin && origin) {
      res.setHeader("Access-Control-Allow-Origin", origin);
      res.setHeader("Vary", "Origin");
    }
    res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type, X-Pi-Question-Token");

    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    if (req.method === "POST" && req.url === "/submit") {
      const requestTokenRaw = req.headers["x-pi-question-token"];
      const requestToken = Array.isArray(requestTokenRaw) ? requestTokenRaw[0] : requestTokenRaw;
      if (requestToken !== sessionToken) {
        sendJson(403, { ok: false, error: "Invalid session token" });
        return;
      }

      let body = "";
      let bodyBytes = 0;
      let rejectedForSize = false;

      req.on("data", (chunk: Buffer | string) => {
        if (rejectedForSize) return;
        bodyBytes += Buffer.byteLength(chunk);
        if (bodyBytes > MAX_RESULT_BODY_BYTES) {
          rejectedForSize = true;
          sendJson(413, { ok: false, error: "Request body too large" });
          req.destroy();
          return;
        }
        body += chunk.toString();
      });

      req.on("end", () => {
        if (rejectedForSize || res.writableEnded) return;
        const parsed = parseBatchSubmission(body);
        if (!parsed.ok) {
          sendJson(parsed.status, { ok: false, error: parsed.error });
          return;
        }

        sendJson(200, { ok: true });
        if (!settled) {
          settled = true;
          resolveResult(parsed.value);
          server.close();
        }
      });

      req.on("error", () => {
        if (!res.writableEnded) {
          sendJson(400, { ok: false, error: "Request error" });
        }
      });
      return;
    }

    res.writeHead(404);
    res.end();
  });

  return new Promise((resolve, reject) => {
    const onError = (error: Error): void => {
      try {
        server.close(() => reject(error));
      } catch {
        reject(error);
      }
    };

    server.once("error", onError);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", onError);
      const addr = server.address();
      if (addr && typeof addr === "object") {
        resolve({ server, port: addr.port, resultPromise });
      } else {
        const error = new Error("Could not determine result server port");
        try {
          server.close();
        } catch {}
        reject(error);
      }
    });
  });
}

function waitForResultWithTimeout(
  resultPromise: Promise<BatchSubmitPayload>,
  timeoutMs = 300000,
): Promise<BatchSubmitPayload> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Timeout waiting for user response")), timeoutMs);
    resultPromise.then(
      (result) => {
        clearTimeout(timeout);
        resolve(result);
      },
      (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });
}

function openHtmlFile(htmlPath: string): void {
  try {
    const platform = process.platform;
    const child =
      platform === "darwin"
        ? spawn("open", [htmlPath], { stdio: "ignore", detached: true })
        : platform === "win32"
          ? spawn("explorer", [htmlPath], { stdio: "ignore", detached: true, windowsHide: true })
          : spawn("xdg-open", [htmlPath], { stdio: "ignore", detached: true });
    child.on("error", () => {});
    child.unref();
  } catch {
    // ignore
  }
}

function previewToMarkdown(preview?: QuestionPreview | string): string | undefined {
  if (!preview) return undefined;
  if (typeof preview === "string") return preview.trim() || undefined;
  if (preview.kind === "mermaid" && preview.content) return `\`\`\`mermaid\n${preview.content}\n\`\`\``;
  if (preview.kind === "code" && preview.content) return `\`\`\`${preview.language ?? ""}\n${preview.content}\n\`\`\``;
  return (
    [preview.title ? `### ${preview.title}` : "", preview.content, preview.url, preview.path]
      .filter(Boolean)
      .join("\n\n")
      .trim() || undefined
  );
}

function makeHeader(question: QuestionItem, index: number): string {
  const raw = question.id || question.i18nKey || question.question || `Q${index + 1}`;
  return (
    raw
      .replace(/[^a-zA-Z0-9 _-]/g, " ")
      .trim()
      .slice(0, 16) || `Q${index + 1}`
  );
}

function optionMatchesRecommendation(option: QuestionOption, recommendedOption?: string): boolean {
  if (!recommendedOption) return false;
  return option.label === recommendedOption || (option.value ?? option.label) === recommendedOption;
}

function toRpivQuestions(questions: QuestionItem[]): { questions: any[] } {
  return {
    questions: questions.map((question, index) => {
      const options = question.options ?? [];
      const recommendedOption = question.recommendedOption;
      const questionPreview = previewToMarkdown(question.preview);
      return {
        question: question.recommendation
          ? `${question.question}\n\nRecommendation: ${question.recommendation}`
          : question.question,
        header: makeHeader(question, index),
        multiSelect: question.type === "multi-select",
        options: options.map((option, optionIndex) => {
          const isRecommended = optionMatchesRecommendation(option, recommendedOption);
          const label =
            isRecommended && !/\(Recommended\)/i.test(option.label) ? `${option.label} (Recommended)` : option.label;
          const notes = [
            ...(option.notes ?? []).map((note) => `${note.title ? `${note.title}: ` : ""}${note.body}`),
            ...(option.description ? [option.description] : []),
          ];
          return {
            label,
            description: notes.join(" ").trim() || option.value || option.label,
            preview: previewToMarkdown(option.preview) ?? (optionIndex === 0 ? questionPreview : undefined),
          };
        }),
      };
    }),
  };
}

export default function question(pi: ExtensionAPI) {
  registerRpivAskUserQuestionTool(pi);

  pi.registerTool({
    name: "AskUserQuestion",
    label: "Ask User Question",
    description:
      "Ask the user a clarifying question in the TUI. Supports single-choice, multi-select, and free-form fill-in-the-blank answers. Optionally include a Mermaid diagram.",
    promptSnippet:
      "Ask the user a clarifying question and wait for the answer before proceeding with ambiguous or destructive work. Add a Mermaid diagram when visual structure would make the decision easier.",
    promptGuidelines: [
      "Use AskUserQuestion instead of guessing when the user must choose between multiple valid paths.",
      "Use AskUserQuestion with type 'single-choice' for one choice, 'multi-select' for multiple choices, or 'text' for fill-in-the-blank answers.",
      "For decision questions, ground the user in one recommended answer: put the recommended option first, append '(Recommended)' to its label when options are shown, and fill the recommendation field with the reason.",
      "Use option descriptions to explain what each choice means and its main trade-off; do not rely on terse labels alone.",
      "Prefer concise, decision-shaping questions with 2-6 options when possible.",
      "Include the optional mermaid field when relationships, architecture, or branching logic are easier to understand visually.",
    ],
    parameters: AskUserQuestionParams,

    async execute(_toolCallId, params, _signal, _onUpdate, ctx: QuestionExecutionContext) {
      const normalized = normalizeQuestionItem({
        question: params.question,
        type: params.type,
        options: params.options,
        allowOther: params.allowOther,
        placeholder: params.placeholder,
        recommendation: params.recommendation,
        recommendedOption: params.recommendedOption,
        preview: params.preview,
        notes: params.notes,
        allowUserNote: params.allowUserNote,
        i18nKey: params.i18nKey,
      });
      const options = normalized.options;
      const questionType = normalized.type;
      const { visual, error: visualError } = await buildMermaidVisual(params.mermaid);
      const prompt = buildPrompt(params.question, visual, visualError, options, params.recommendation);
      const detailsBase = {
        question: params.question,
        questionType,
        options: options.map((option) => option.label),
        mermaid: visual,
        visualError: visualError ?? null,
        recommendation: params.recommendation ?? null,
        recommendedOption: params.recommendedOption ?? null,
        preview: params.preview ?? null,
        notes: params.notes ?? [],
        i18n: params.i18n ?? null,
      };

      if (visual) emitPiMermaidMessage(pi, visual);

      if (questionType === "decision") {
        if (options.length < 2 || options.length > 4) {
          return {
            content: [{ type: "text", text: "Error: decision questions require 2-4 options." }],
            details: {
              ...detailsBase,
              answer: null,
              value: null,
              selectedLabels: null,
              selectedValues: null,
              cancelled: true,
              mode: "decision",
            } satisfies AskUserQuestionDetails,
          };
        }
        if (
          params.recommendedOption &&
          !options.some((option) => optionMatchesRecommendation(option, params.recommendedOption))
        ) {
          return {
            content: [{ type: "text", text: "Error: recommendedOption must match an option label or value." }],
            details: {
              ...detailsBase,
              answer: null,
              value: null,
              selectedLabels: null,
              selectedValues: null,
              cancelled: true,
              mode: "decision",
            } satisfies AskUserQuestionDetails,
          };
        }
      }

      if (!ctx.hasUI) {
        return {
          content: [{ type: "text", text: "UI not available; could not ask the user question." }],
          details: {
            ...detailsBase,
            answer: null,
            value: null,
            selectedLabels: null,
            selectedValues: null,
            cancelled: true,
            mode: "unavailable",
          } satisfies AskUserQuestionDetails,
        };
      }

      if (questionType === "text" || options.length === 0) {
        const answer = await readTrimmedInput(ctx, prompt, params.placeholder);
        if (answer === undefined) {
          return {
            content: [{ type: "text", text: "User cancelled the question." }],
            details: {
              ...detailsBase,
              answer: null,
              value: null,
              selectedLabels: null,
              selectedValues: null,
              cancelled: true,
              mode: "input",
            } satisfies AskUserQuestionDetails,
          };
        }

        return {
          content: [{ type: "text", text: `User answered: ${answer}` }],
          details: {
            ...detailsBase,
            answer,
            value: answer,
            selectedLabels: null,
            selectedValues: null,
            cancelled: false,
            mode: "input",
          } satisfies AskUserQuestionDetails,
        };
      }

      const allowOther = questionType === "decision" ? false : params.allowOther !== false;

      if (questionType === "multi-select") {
        const result = await askMultiSelectInTui(ctx, prompt, options, allowOther, params.placeholder);
        if (result.cancelled) {
          return {
            content: [{ type: "text", text: "User cancelled the question." }],
            details: {
              ...detailsBase,
              answer: null,
              value: null,
              selectedLabels: null,
              selectedValues: null,
              cancelled: true,
              mode: "multi-select",
            } satisfies AskUserQuestionDetails,
          };
        }

        return {
          content: [{ type: "text", text: `User selected: ${result.labels.join(", ")}` }],
          details: {
            ...detailsBase,
            answer: result.labels,
            value: result.values,
            selectedLabels: result.labels,
            selectedValues: result.values,
            cancelled: false,
            mode: "multi-select",
          } satisfies AskUserQuestionDetails,
        };
      }

      const labels = allowOther
        ? [...options.map((option) => option.label), OTHER_LABEL]
        : options.map((option) => option.label);
      const selection = await ctx.ui.select(prompt, labels);
      if (selection === undefined) {
        return {
          content: [{ type: "text", text: "User cancelled the question." }],
          details: {
            ...detailsBase,
            answer: null,
            value: null,
            selectedLabels: null,
            selectedValues: null,
            cancelled: true,
            mode: questionType === "decision" ? "decision" : "select",
          } satisfies AskUserQuestionDetails,
        };
      }

      if (selection === OTHER_LABEL) {
        const answer = await readTrimmedInput(ctx, prompt, params.placeholder);
        if (answer === undefined) {
          return {
            content: [{ type: "text", text: "User cancelled the question." }],
            details: {
              ...detailsBase,
              answer: null,
              value: null,
              selectedLabels: null,
              selectedValues: null,
              cancelled: true,
              mode: "other",
            } satisfies AskUserQuestionDetails,
          };
        }

        return {
          content: [{ type: "text", text: `User answered: ${answer}` }],
          details: {
            ...detailsBase,
            answer,
            value: answer,
            selectedLabels: null,
            selectedValues: null,
            cancelled: false,
            mode: "other",
          } satisfies AskUserQuestionDetails,
        };
      }

      const matched = options.find((option) => option.label === selection) ?? { label: selection, value: selection };
      return {
        content: [{ type: "text", text: `User selected: ${matched.label}` }],
        details: {
          ...detailsBase,
          answer: matched.label,
          value: matched.value ?? matched.label,
          selectedLabels: [matched.label],
          selectedValues: [matched.value ?? matched.label],
          cancelled: false,
          mode: questionType === "decision" ? "decision" : "select",
        } satisfies AskUserQuestionDetails,
      };
    },

    renderCall(args, theme: QuestionTheme) {
      const optionLabels = Array.isArray(args.options) ? args.options.map((o: { label: string }) => o.label) : [];
      const questionType = normalizeQuestionType(
        args.type as QuestionType | undefined,
        args.options as QuestionOption[] | undefined,
      );
      let text = theme.fg("toolTitle", theme.bold("AskUserQuestion ")) + theme.fg("muted", args.question);
      text += "\n" + theme.fg("dim", `  Type: ${questionType}`);
      if (typeof args.mermaid === "string" && args.mermaid.trim()) {
        text += "\n" + theme.fg("accent", "  Visual: Mermaid diagram included");
      }
      if (args.recommendation) {
        text += "\n" + theme.fg("info", "  💡 " + args.recommendation);
      }
      if (optionLabels.length > 0) {
        const suffix =
          args.allowOther === false
            ? ""
            : questionType === "multi-select"
              ? ", Type something... (optional extra)"
              : ", Type something...";
        text += "\n" + theme.fg("dim", `  Options: ${optionLabels.join(", ")}${suffix}`);
      }
      return new Text(text, 0, 0);
    },

    renderResult(result, options, theme: QuestionTheme) {
      const details = result.details as AskUserQuestionDetails | undefined;
      if (!details) {
        const text = result.content[0];
        return new Text(text?.type === "text" ? text.text : "", 0, 0);
      }
      if (details.cancelled) {
        return new Text(buildResultText(theme.fg("warning", "Cancelled"), details, options.expanded, theme), 0, 0);
      }
      return new Text(
        buildResultText(
          theme.fg("success", "✓ ") + theme.fg("accent", formatAnswer(details.answer)),
          details,
          options.expanded,
          theme,
        ),
        0,
        0,
      );
    },
  });

  pi.registerTool({
    name: "AskBatchQuestions",
    label: "Ask Batch Questions",
    description:
      "Ask multiple questions at once. Defaults to the legacy browser UI; supports an opt-in rpiv-style TUI with tabs, previews, notes, and strict 2-4 option questions via presentation:'tui'.",
    promptSnippet:
      "Ask multiple questions at once when the user needs to make several related decisions. Use presentation:'tui' for terminal-native interviews with previews and notes.",
    promptGuidelines: [
      "Use AskBatchQuestions when you need to ask 2 or more related questions.",
      "Use AskBatchQuestions for mixed questionnaires that include single-choice, multi-select, and text questions.",
      "Use presentation:'tui' when you want the rpiv-style terminal questionnaire: tabs, previews, notes, multi-select, and submit review.",
      "For decision questions, provide 2-4 options, put the recommended answer first, and set recommendedOption plus recommendation.",
      "Include recommendations for each question based on scope and context when possible.",
      "The user will see all questions at once and can answer them in any order.",
    ],
    parameters: AskBatchQuestionsParams,

    async execute(_toolCallId, params, _signal, _onUpdate, ctx: QuestionExecutionContext) {
      const presentation = (params.presentation ?? "browser") as Presentation;
      if (presentation === "tui" || (presentation === "auto" && ctx.hasUI)) {
        const rpivResult = await executeAskUserQuestionnaire(
          pi,
          toRpivQuestions(params.questions as QuestionItem[]),
          ctx,
        );
        const answers = Array.isArray(rpivResult.details?.answers)
          ? rpivResult.details.answers.map((answer) => ({
              question: answer.question,
              questionType: answer.kind === "multi" ? "multi-select" : "single-choice",
              answer: answer.kind === "multi" ? (answer.selected ?? []) : answer.answer,
              value: answer.kind === "multi" ? (answer.selected ?? []) : answer.answer,
              selectedLabel: answer.kind === "option" ? answer.answer : null,
              selectedLabels:
                answer.kind === "multi" ? (answer.selected ?? []) : answer.answer ? [answer.answer] : null,
              selectedValues:
                answer.kind === "multi" ? (answer.selected ?? []) : answer.answer ? [answer.answer] : null,
              isCustomInput: answer.kind === "custom" || answer.kind === "chat",
            }))
          : [];
        return {
          content: rpivResult.content,
          details: {
            title: params.title,
            description: params.description ?? null,
            questions: answers,
            cancelled: Boolean(rpivResult.details?.cancelled),
            presentation,
          } satisfies AskBatchQuestionsDetails,
        };
      }

      if (!ctx.hasUI) {
        return {
          content: [{ type: "text", text: "UI not available; could not ask batch questions." }],
          details: {
            title: params.title,
            description: params.description ?? null,
            questions: [],
            cancelled: true,
            presentation,
          } satisfies AskBatchQuestionsDetails,
        };
      }

      const sessionId = randomUUID().slice(0, 8);
      const sessionToken = randomUUID();
      const expiresAt = Date.now() + QUESTIONNAIRE_TIMEOUT_MS;
      const htmlPath = join(tmpdir(), `pi-questions-${sessionId}.html`);
      let server: Server | null = null;

      try {
        const resultServer = await startResultServer(sessionToken);
        server = resultServer.server;

        const html = generateBatchQuestionsHTML({
          title: params.title,
          description: params.description,
          questions: params.questions as QuestionItem[],
          serverPort: resultServer.port,
          sessionToken,
          expiresAt,
        });

        writeFileSync(htmlPath, html, "utf8");
        ctx.ui.notify("Interactive questionnaire opened in browser. Answer all questions and click Submit.", "info");
        openHtmlFile(htmlPath);

        const result = await waitForResultWithTimeout(resultServer.resultPromise, QUESTIONNAIRE_TIMEOUT_MS);
        try {
          unlinkSync(htmlPath);
        } catch {}
        if (server) {
          try {
            server.close();
          } catch {}
        }

        if (result.cancelled) {
          return {
            content: [{ type: "text", text: "User cancelled the questionnaire." }],
            details: {
              title: params.title,
              description: params.description ?? null,
              questions: [],
              cancelled: true,
              presentation,
            } satisfies AskBatchQuestionsDetails,
          };
        }

        const summaryLines = [
          `User answered ${result.answers.length} questions:`,
          ...result.answers.map((answer, index) => `${index + 1}. ${answer.question} → ${formatAnswer(answer.answer)}`),
        ];

        return {
          content: [{ type: "text", text: summaryLines.join("\n") }],
          details: {
            title: params.title,
            description: params.description ?? null,
            questions: result.answers,
            cancelled: false,
            presentation,
          } satisfies AskBatchQuestionsDetails,
        };
      } catch (error) {
        try {
          unlinkSync(htmlPath);
        } catch {}
        if (server) {
          try {
            server.close();
          } catch {}
        }

        return {
          content: [{ type: "text", text: `Error: ${error instanceof Error ? error.message : String(error)}` }],
          details: {
            title: params.title,
            description: params.description ?? null,
            questions: [],
            cancelled: true,
            presentation,
          } satisfies AskBatchQuestionsDetails,
        };
      }
    },

    renderCall(args, theme: QuestionTheme) {
      const questionCount = args.questions?.length ?? 0;
      let text = theme.fg("toolTitle", theme.bold("AskBatchQuestions ")) + theme.fg("muted", args.title);
      text += "\n" + theme.fg("dim", `  Questions: ${questionCount}`);
      if (args.description) text += "\n" + theme.fg("muted", `  ${args.description}`);
      return new Text(text, 0, 0);
    },

    renderResult(result, _options, theme: QuestionTheme) {
      const details = result.details as AskBatchQuestionsDetails | undefined;
      if (details?.cancelled) return new Text(theme.fg("warning", "Cancelled"), 0, 0);
      if (!details || details.questions.length === 0) return new Text(theme.fg("warning", "No answers received"), 0, 0);

      const summaryLines = [
        theme.fg("success", `✓ Answered ${details.questions.length} questions:`),
        ...details.questions.map(
          (answer, index) =>
            theme.fg("accent", `${index + 1}. ${answer.question}`) +
            " → " +
            theme.fg("success", formatAnswer(answer.answer)),
        ),
      ];
      return new Text(summaryLines.join("\n"), 0, 0);
    },
  });
}
