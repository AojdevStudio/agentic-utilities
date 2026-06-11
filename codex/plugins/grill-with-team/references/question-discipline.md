# Question Discipline

Ask fewer, harder questions.

## Question Gate

Before asking the user, check:

1. Is this already resolved in `CONTEXT.html`, ADRs, tracker docs, or the plan?
2. Is it answerable from code or local docs through Explorer?
3. Is it material to a downstream decision?
4. Is it blocking the next branch?
5. Is there a safe default that can be recorded as an assumption?

Ask only when the answer changes the domain model, implementation plan, issue breakdown, or ADR trail.

## Question Shape

- Ask one question at a time.
- Lead with one recommended answer.
- Explain the trade-off briefly.
- Use the repo's glossary vocabulary exactly.
- Fold in the strongest RedTeam/Cato objection before asking.

## ADR-Worthiness Test

Create an HTML ADR only when all three are true:

1. The decision is hard to reverse.
2. The decision will be surprising without context.
3. The decision is the result of a real trade-off.

If any one is missing, keep the decision state in `CONTEXT.html` and skip the ADR.
