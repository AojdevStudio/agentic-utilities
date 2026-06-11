# Question Extension User Testing

Use `pi -e .` from this repo, then ask the agent to call each tool shape below. Do not paste local paths, private hostnames, or real credentials into results.

## Matrix

### 1. Legacy single-choice with recommendation

Expected: TUI prompt shows recommendation and option descriptions.

```ts
AskUserQuestion({
  question: "Decision: how should review metadata be represented?",
  type: "decision",
  recommendedOption: "manifest",
  recommendation: "Use SKILL.md + manifest because it keeps the prose portable while giving Pi reliable routing/scoring knobs.",
  allowOther: false,
  options: [
    { label: "SKILL.md + manifest (Recommended)", value: "manifest", description: "Best production behavior; slightly more structure to maintain." },
    { label: "Pure SKILL.md", value: "skill-only", description: "Best portability; weaker machine routing/scoring." },
    { label: "Plain rubric", value: "rubric", description: "Simplest parser; loses Agent Skills portability." }
  ]
})
```

### 2. Legacy text input

Expected: blank answers are rejected; final answer returns `details.answer` and `details.value`.

```ts
AskUserQuestion({ question: "What short label should this workflow use?", type: "text" })
```

### 3. Legacy multi-select

Expected: selectable checkboxes through repeated TUI picker, plus optional custom text.

```ts
AskUserQuestion({
  question: "Which validation gates should run?",
  type: "multi-select",
  options: [
    { label: "typecheck", description: "Run TypeScript validation." },
    { label: "lint", description: "Run Biome lint." },
    { label: "smoke tests", description: "Run focused smoke tests." }
  ]
})
```

### 4. Browser batch compatibility

Expected: browser questionnaire opens; this remains the default when `presentation` is omitted.

```ts
AskBatchQuestions({
  title: "Question browser smoke",
  questions: [
    { question: "Pick an implementation style", options: [{ label: "Small", description: "Minimal patch." }, { label: "Full", description: "Broader refactor." }] },
    { question: "Add docs?", options: [{ label: "Yes", description: "Document behavior." }, { label: "No", description: "Code only." }] }
  ]
})
```

### 5. TUI batch / rpiv-style questionnaire

Expected: terminal overlay with tabs, option descriptions, preview pane, submit review, and cancel/chat escape.

```ts
AskBatchQuestions({
  title: "Question TUI smoke",
  presentation: "tui",
  questions: [
    {
      question: "Which API shape should be canonical?",
      type: "decision",
      recommendedOption: "compatible",
      recommendation: "Keep legacy tools and add snake_case alias to avoid breaking skills.",
      preview: { kind: "markdown", content: "```ts\nAskUserQuestion(...)\nask_user_question(...)\n```" },
      options: [
        { label: "Compatible alias (Recommended)", value: "compatible", description: "Best migration path." },
        { label: "Replace old tools", value: "replace", description: "Cleaner API but breaking." }
      ]
    },
    {
      question: "Which extras should ship?",
      type: "multi-select",
      options: [
        { label: "previews", description: "Show side-by-side context." },
        { label: "notes", description: "Let users annotate choices." },
        { label: "i18n", description: "Use optional localized chrome." }
      ]
    }
  ]
})
```

### 6. rpiv-compatible alias

Expected: same TUI overlay as upstream `@juicesharp/rpiv-ask-user-question`.

```ts
ask_user_question({
  questions: [
    {
      question: "Which review lens should run first?",
      header: "Lens",
      options: [
        { label: "Correctness (Recommended)", description: "Find behavior bugs before polish.", preview: "Focus: regressions, edge cases, invariants." },
        { label: "Maintainability", description: "Find naming, duplication, and structure issues." }
      ]
    }
  ]
})
```

## Results

- Environment: pending
- Pi version: pending
- Automated checks: pending
- Manual result: pending
