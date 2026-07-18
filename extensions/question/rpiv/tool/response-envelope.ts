import { hasBoundedStrings, QUESTION_LIMITS } from "../../limits.js";
import { formatAnswerScalar } from "./format-answer.js";
import {
  MAX_CUSTOM_ANSWER_LENGTH,
  MAX_LABEL_LENGTH,
  MAX_NOTES_LENGTH,
  MAX_PREVIEW_LENGTH,
  MAX_QUESTION_LENGTH,
  MAX_QUESTIONS,
  type QuestionAnswer,
  type QuestionnaireResult,
  type QuestionParams,
} from "./types.js";

export const DECLINE_MESSAGE = "User declined to answer questions";
export const ENVELOPE_PREFIX = "User has answered your questions:";
export const ENVELOPE_SUFFIX = "You can now continue with the user's answers in mind.";
export const RESULT_TOO_LARGE_MESSAGE = "Error: questionnaire result exceeds configured output limits";

export function validateQuestionnaireResult(result: QuestionnaireResult): boolean {
  if (result.answers.length > MAX_QUESTIONS) return false;
  for (const answer of result.answers) {
    if (answer.question.length > MAX_QUESTION_LENGTH) return false;
    if ((answer.answer?.length ?? 0) > MAX_CUSTOM_ANSWER_LENGTH) return false;
    if ((answer.notes?.length ?? 0) > MAX_NOTES_LENGTH) return false;
    if ((answer.preview?.length ?? 0) > MAX_PREVIEW_LENGTH) return false;
    if ((answer.selected?.length ?? 0) > QUESTION_LIMITS.optionsPerQuestion) return false;
    if (answer.selected?.some((label) => label.length > MAX_LABEL_LENGTH)) return false;
  }
  const userOutput = result.answers.map(({ answer, selected, notes }) => ({ answer, selected, notes }));
  return (
    hasBoundedStrings(userOutput, QUESTION_LIMITS.totalUserOutput) &&
    hasBoundedStrings(result, QUESTION_LIMITS.toolDetails)
  );
}

/**
 * Map a `QuestionnaireResult` (or null/cancelled) to the LLM-facing tool envelope.
 * Pure of `(result, params)`; cancelled and "no segments" both fall to `DECLINE_MESSAGE`
 * so the model sees a single canonical "didn't answer" signal regardless of why.
 */
export function buildQuestionnaireResponse(result: QuestionnaireResult | null | undefined, params: QuestionParams) {
  if (!result || result.cancelled) {
    return buildToolResult(DECLINE_MESSAGE, {
      answers: result?.answers ?? [],
      cancelled: true,
    });
  }
  const segments: string[] = [];
  for (let i = 0; i < params.questions.length; i++) {
    const a = result.answers.find((x) => x.questionIndex === i);
    if (a) segments.push(buildAnswerSegment(a));
  }
  if (segments.length === 0) {
    return buildToolResult(DECLINE_MESSAGE, { answers: result.answers, cancelled: true });
  }
  return buildToolResult(`${ENVELOPE_PREFIX} ${segments.join(" ")} ${ENVELOPE_SUFFIX}`, result);
}

/**
 * Format a single answer segment for the envelope. Pure of `a`. The `"Q"="A"` shape and
 * the optional `selected preview:` / `user notes:` suffixes are pinned by envelope tests.
 */
export function buildAnswerSegment(a: QuestionAnswer): string {
  const parts: string[] = [`"${a.question}"="${formatAnswerScalar(a, "envelope")}"`];
  if (a.preview && a.preview.length > 0) parts.push(`selected preview: ${a.preview}`);
  if (a.notes && a.notes.length > 0) parts.push(`user notes: ${a.notes}`);
  return `${parts.join(". ")}.`;
}

export function buildToolResult(text: string, details: QuestionnaireResult) {
  if (text.length > QUESTION_LIMITS.toolContent || !validateQuestionnaireResult(details)) {
    return {
      content: [{ type: "text" as const, text: RESULT_TOO_LARGE_MESSAGE }],
      details: { answers: [], cancelled: true, error: "result_too_large" as const },
    };
  }
  return {
    content: [{ type: "text" as const, text }],
    details,
  };
}
