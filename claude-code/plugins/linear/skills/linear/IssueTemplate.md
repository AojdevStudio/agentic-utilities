# Linear Issue Template

> Every issue created through this skill must follow this template.
> Use CreateIssue or PlanProject workflows — never create issues ad-hoc.

## Template

### Summary
1-2 sentences: what gets accomplished and why.

### Context
- **Repository:** `<local path from linear-context.json>`
- **GitHub:** `<org/repo from linear-context.json>`
- **Relevant Files:** `<comma-separated file paths, or "To be determined during investigation">`
- **Related Issues:** `<identifiers or "None">`

### Current Behavior
What happens now. Use "N/A" for new features.

### Expected Behavior
What should happen after this issue is resolved.

### Acceptance Criteria (max 3)
- [ ] AC-1: [Testable outcome — one sentence, binary pass/fail]
- [ ] AC-2: [Testable outcome]
- [ ] AC-3: [Testable outcome]

### Scope
**In scope:** [explicit inclusions]
**Out of scope:** [explicit exclusions]

### Additional Context
Logs, screenshots, error messages, design links.

---

## Rules

1. **Max 3 acceptance criteria.** If more are needed, the issue is too large — decompose it.
2. **Binary testable.** Each AC must be verifiable with a pass/fail test.
3. **No implementation details in AC.** Describe *what*, not *how*.
4. **Scope boundaries are mandatory.** Explicitly state what is NOT included.
5. **Context section is mandatory for code projects.** Repository and GitHub fields are auto-populated from `linear-context.json`.
6. **Relevant Files should list specific paths when known**, or "To be determined" for exploratory issues.
7. **Self-contained.** An AI agent should be able to resolve this issue without asking clarifying questions.

## Examples

### Bug Fix

```markdown
## Summary
Fix the patient search returning duplicate results when filtering by insurance provider.

## Context
- **Repository:** `~/Projects/my-app`
- **GitHub:** `<org>/my-app`
- **Relevant Files:** `src/api/patients/search.ts, src/components/PatientSearch.tsx`
- **Related Issues:** `<TEAM>-89`

## Current Behavior
Searching for "Delta Dental" returns each patient twice — once per coverage record.

## Expected Behavior
Search returns unique patients, with coverage details aggregated per patient.

## Acceptance Criteria (max 3)
- [ ] AC-1: Patient search returns no duplicate patient rows
- [ ] AC-2: Coverage details display as list within each patient row
- [ ] AC-3: Search performance under 500ms for 1000+ patients

## Scope
**In scope:** Patient search API endpoint, search results component
**Out of scope:** Insurance provider management, patient creation flow
```

### Feature

```markdown
## Summary
Add email notification when a claim status changes so office managers stay informed without checking the dashboard.

## Context
- **Repository:** `~/Projects/my-app`
- **GitHub:** `<org>/my-app`
- **Relevant Files:** `To be determined during investigation`
- **Related Issues:** `None`

## Current Behavior
N/A — no notifications exist.

## Expected Behavior
Email sent to office manager within 5 minutes of claim status change.

## Acceptance Criteria (max 3)
- [ ] AC-1: Email sent on status transitions: submitted->processing, processing->approved, processing->denied
- [ ] AC-2: Email contains claim ID, patient name, old status, new status
- [ ] AC-3: Office manager email pulled from organization settings

## Scope
**In scope:** Claim status change event, email template, send logic
**Out of scope:** SMS notifications, in-app notifications, notification preferences UI
```

---

*Referenced by: ResolveIssue, PlanProject, CreateIssue workflows*
