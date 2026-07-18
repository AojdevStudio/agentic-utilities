export const QUESTION_LIMITS = {
  questionText: 1_000,
  title: 200,
  description: 2_000,
  optionLabel: 60,
  optionValue: 200,
  optionDescription: 1_000,
  previewTitle: 200,
  previewContent: 8_000,
  previewMetadata: 500,
  noteTitle: 200,
  noteBody: 2_000,
  placeholder: 200,
  recommendation: 2_000,
  i18nKey: 200,
  i18nValue: 2_000,
  i18nEntries: 64,
  optionsPerQuestion: 20,
  batchQuestions: 12,
  customAnswer: 4_000,
  totalQuestionnaireInput: 32_000,
  totalUserOutput: 16_000,
  toolContent: 32_000,
  toolDetails: 64_000,
} as const;

/** Sum all string values (and object keys) without serializing attacker-controlled objects. */
export function totalStringLength(value: unknown, seen = new Set<object>()): number {
  if (typeof value === "string") return value.length;
  if (value === null || typeof value !== "object") return 0;
  if (seen.has(value)) return 0;
  seen.add(value);

  if (Array.isArray(value)) {
    return value.reduce((total, item) => total + totalStringLength(item, seen), 0);
  }

  let total = 0;
  for (const [key, item] of Object.entries(value)) {
    total += key.length + totalStringLength(item, seen);
  }
  return total;
}

export function hasBoundedStrings(value: unknown, maximum: number): boolean {
  return totalStringLength(value) <= maximum;
}

export function clampInput(value: string, maximum: number): string {
  return value.length <= maximum ? value : value.slice(0, maximum);
}

export function isBoundedRecord(
  value: unknown,
  maximumEntries = QUESTION_LIMITS.i18nEntries,
  maximumKeyLength = QUESTION_LIMITS.i18nKey,
  maximumValueLength = QUESTION_LIMITS.i18nValue,
): value is Record<string, string> {
  if (value === undefined) return true;
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const entries = Object.entries(value);
  return (
    entries.length <= maximumEntries &&
    entries.every(
      ([key, entryValue]) =>
        key.length <= maximumKeyLength && typeof entryValue === "string" && entryValue.length <= maximumValueLength,
    )
  );
}
