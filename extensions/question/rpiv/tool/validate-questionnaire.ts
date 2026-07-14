import { hasBoundedStrings, QUESTION_LIMITS } from "../../limits.js";
import {
  MAX_DESCRIPTION_LENGTH,
  MAX_HEADER_LENGTH,
  MAX_LABEL_LENGTH,
  MAX_OPTIONS,
  MAX_PREVIEW_LENGTH,
  MAX_QUESTION_LENGTH,
  MAX_QUESTIONS,
  MIN_OPTIONS,
  type QuestionnaireError,
  type QuestionParams,
  RESERVED_LABELS,
} from "./types.js";

export const ERROR_NO_QUESTIONS = "Error: At least one question is required";
export const ERROR_TOO_MANY_QUESTIONS = `Error: At most ${MAX_QUESTIONS} questions are allowed per invocation`;
export const ERROR_DUPLICATE_QUESTION = "Error: Question text must be unique within an invocation";
export const ERROR_TOO_FEW_OPTIONS = `Error: Each question requires at least ${MIN_OPTIONS} options`;
export const ERROR_TOO_MANY_OPTIONS = `Error: Each question allows at most ${MAX_OPTIONS} options`;
export const ERROR_RESERVED_LABEL = `Error: Option label is reserved (${RESERVED_LABELS.join(", ")})`;
export const ERROR_DUPLICATE_OPTION_LABEL = "Error: Option labels must be unique within a question";
export const ERROR_INVALID_LENGTH = "Error: Questionnaire field exceeds its maximum length";
export const ERROR_INPUT_TOO_LARGE = `Error: Questionnaire exceeds the ${QUESTION_LIMITS.totalQuestionnaireInput}-character aggregate limit`;

const RESERVED_LABEL_SET: ReadonlySet<string> = new Set(RESERVED_LABELS);

export type ValidationResult = { ok: true } | { ok: false; error: QuestionnaireError; message: string };

/**
 * Pure runtime validator for `QuestionParams`. Covers every guard except
 * `no_ui` (which depends on `ctx.hasUI` and stays inline at the call site).
 * `reserved_label` MUST short-circuit before `duplicate_option_label`.
 */
export function validateQuestionnaire(typed: QuestionParams): ValidationResult {
  if (typed.questions.length === 0) {
    return { ok: false, error: "no_questions", message: ERROR_NO_QUESTIONS };
  }
  if (typed.questions.length > MAX_QUESTIONS) {
    return { ok: false, error: "too_many_questions", message: ERROR_TOO_MANY_QUESTIONS };
  }
  if (!hasBoundedStrings(typed, QUESTION_LIMITS.totalQuestionnaireInput)) {
    return { ok: false, error: "input_too_large", message: ERROR_INPUT_TOO_LARGE };
  }

  const seenQuestions = new Set<string>();
  for (const q of typed.questions) {
    if (q.question.length > MAX_QUESTION_LENGTH || q.header.length > MAX_HEADER_LENGTH) {
      return { ok: false, error: "invalid_length", message: ERROR_INVALID_LENGTH };
    }
    if (seenQuestions.has(q.question)) {
      return { ok: false, error: "duplicate_question", message: ERROR_DUPLICATE_QUESTION };
    }
    seenQuestions.add(q.question);
  }

  for (const q of typed.questions) {
    if (q.options.length < MIN_OPTIONS) {
      return { ok: false, error: "empty_options", message: ERROR_TOO_FEW_OPTIONS };
    }
    if (q.options.length > MAX_OPTIONS) {
      return { ok: false, error: "too_many_options", message: ERROR_TOO_MANY_OPTIONS };
    }
    const seenLabels = new Set<string>();
    for (const o of q.options) {
      if (
        o.label.length > MAX_LABEL_LENGTH ||
        o.description.length > MAX_DESCRIPTION_LENGTH ||
        (o.preview?.length ?? 0) > MAX_PREVIEW_LENGTH
      ) {
        return { ok: false, error: "invalid_length", message: ERROR_INVALID_LENGTH };
      }
      if (RESERVED_LABEL_SET.has(o.label)) {
        return { ok: false, error: "reserved_label", message: ERROR_RESERVED_LABEL };
      }
      if (seenLabels.has(o.label)) {
        return {
          ok: false,
          error: "duplicate_option_label",
          message: ERROR_DUPLICATE_OPTION_LABEL,
        };
      }
      seenLabels.add(o.label);
    }
  }

  return { ok: true };
}
