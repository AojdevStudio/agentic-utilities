---
name: grill-with-docs
description: Grilling session that challenges your plan against the existing domain model, sharpens terminology, and updates documentation (CONTEXT.md, ADRs) inline as decisions crystallise. Use when user wants to stress-test a plan against their project's language and documented decisions.
---

<what-to-do>

Interview me relentlessly about every aspect of this plan until we reach a shared understanding. Walk down each branch of the design tree, resolving dependencies between decisions one-by-one. For each decision, ground the discussion in one recommended answer instead of opening the option space too wide.

Ask the questions one at a time using the AskUserQuestion extension for any multiple-choice or decision point, waiting for feedback on each question before continuing.

If a question can be answered by exploring the codebase, explore the codebase instead.

Decision questions must follow this shape before calling AskUserQuestion:

```text
Decision: <the specific decision being made>

Recommendation: <one best/default answer and why it fits this repo/plan>

Options:
A. <Recommended option> (Recommended)
   <When it is best. Main trade-off.>
B. <Viable alternative>
   <When it is best. Main trade-off.>
C. <Only include if genuinely useful>
   <When it is best. Main trade-off.>
```

AskUserQuestion requirements:
- Use `type: "decision"` for decision questions.
- Put the recommended answer first.
- Add `(Recommended)` to the recommended option label when options are shown.
- Populate `recommendedOption` with the recommended option's stable `value` or label.
- Populate each option's `description` with the trade-off, not a vague label.
- Populate the `recommendation` field with the best answer and reason.
- Prefer 2-3 options. Use 4 only when the fourth is genuinely distinct.
- Do not ask vague questions like "what do you mean?" until you have explained the likely interpretations and your default recommendation.

</what-to-do>

<supporting-info>

## Domain awareness

During codebase exploration, also look for existing documentation:

### File structure

Most repos have a single context:

```
/
├── CONTEXT.md
├── docs/
│   └── adr/
│       ├── 0001-event-sourced-orders.md
│       └── 0002-postgres-for-write-model.md
└── src/
```

If a `CONTEXT-MAP.md` exists at the root, the repo has multiple contexts. The map points to where each one lives:

```
/
├── CONTEXT-MAP.md
├── docs/
│   └── adr/                          ← system-wide decisions
├── src/
│   ├── ordering/
│   │   ├── CONTEXT.md
│   │   └── docs/adr/                 ← context-specific decisions
│   └── billing/
│       ├── CONTEXT.md
│       └── docs/adr/
```

Create files lazily — only when you have something to write. If no `CONTEXT.md` exists, create one when the first term is resolved. If no `docs/adr/` exists, create it when the first ADR is needed.

## During the session

### Maintain a decision ledger

Before asking any question, build and maintain a concise **Decision Ledger** from:
- the user's plan/spec/artifact
- prior answers in the current conversation
- repo docs and ADRs
- verified codebase facts

For each decision, track:

```txt
Decision:
Status: resolved | unresolved | contradicted | assumed
Source: user | plan | code | docs | ADR
Confidence: high | medium | low
Notes:
```

Do not store implementation decisions in CONTEXT.md; keep the ledger in conversation or a temporary scratch artifact. CONTEXT.md is only for domain language.

### Question gate

Before asking a question, run this gate:

1. Already resolved?
   If yes, do not ask. State the resolved decision only if needed.
2. Can code/docs answer it?
   If yes, inspect code/docs instead of asking.
3. Is it material?
   Only ask if the answer changes architecture, UX, data model, security, deployment, or implementation order.
4. Is it blocking a downstream branch?
   Ask only when the next useful design branch depends on it.
5. Can it be safely assumed?
   If yes, make a recommendation, mark it as an assumption, and continue.

Only ask questions that pass all gates.

### Start by extracting resolved decisions

When the user provides a plan/spec/artifact, first extract:
- explicit decisions
- implied decisions
- open questions
- contradictions
- assumptions

Then ask only about true gaps. Never re-ask a branch already decided unless new evidence contradicts it.

### Handle user corrections

If the user says a question was already answered, immediately:
1. acknowledge the miss
2. add the decision to the ledger
3. continue from the next unresolved branch

### Challenge against the glossary

When the user uses a term that conflicts with the existing language in `CONTEXT.md`, call it out immediately. "Your glossary defines 'cancellation' as X, but you seem to mean Y — which is it?"

### Sharpen fuzzy language

When the user uses vague or overloaded terms, propose a precise canonical term. "You're saying 'account' — do you mean the Customer or the User? Those are different things."

### Discuss concrete scenarios

When domain relationships are being discussed, stress-test them with specific scenarios. Invent scenarios that probe edge cases and force the user to be precise about the boundaries between concepts.

### Cross-reference with code

When the user states how something works, check whether the code agrees. If you find a contradiction, surface it: "Your code cancels entire Orders, but you just said partial cancellation is possible — which is right?"

### Update CONTEXT.md inline

When a term is resolved, update `CONTEXT.md` right there. Don't batch these up — capture them as they happen. Use the format in [CONTEXT-FORMAT.md](./CONTEXT-FORMAT.md).

`CONTEXT.md` should be totally devoid of implementation details. Do not treat `CONTEXT.md` as a spec, a scratch pad, or a repository for implementation decisions. It is a glossary and nothing else.

### Offer ADRs sparingly

Only offer to create an ADR when all three are true:

1. **Hard to reverse** — the cost of changing your mind later is meaningful
2. **Surprising without context** — a future reader will wonder "why did they do it this way?"
3. **The result of a real trade-off** — there were genuine alternatives and you picked one for specific reasons

If any of the three is missing, skip the ADR. Use the format in [ADR-FORMAT.md](./ADR-FORMAT.md).

</supporting-info>
