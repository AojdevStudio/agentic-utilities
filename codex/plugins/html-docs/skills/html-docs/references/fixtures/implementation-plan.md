# Offline Billing Sync Plan

We need a browser-readable plan for shipping offline billing sync without mixing queue behavior, data migration, and reconciliation policy into one flat thread.

## Goals

- Preserve existing online invoice creation behavior.
- Add local draft persistence for clinics with unstable connectivity.
- Sync queued invoices in creation order once connectivity returns.
- Make reconciliation errors visible before payment capture.

## Milestones

| Milestone | Owner | Exit Criteria |
| --- | --- | --- |
| Data model | Backend | `offline_invoice_drafts` table merged with retention policy |
| Client queue | Web | Draft queue stores create/update/delete events locally |
| Sync worker | Platform | Worker replays events idempotently and records conflicts |
| Operator review | Support | Conflict dashboard shows patient, invoice, and suggested action |

## Data Flow

```text
Browser draft -> IndexedDB queue -> sync endpoint -> idempotency key -> invoice service -> reconciliation log
```

## Risks

- Duplicate invoice creation if idempotency keys are generated after reconnect.
- Stale patient insurance data if a draft is older than the verification window.
- Support overload if every retry becomes a manual conflict.

## Acceptance

- [ ] A draft can be created offline and synced after reconnect.
- [ ] Duplicate submissions reuse the first committed invoice.
- [ ] Conflicts produce a review item, not a silent retry loop.
- [ ] Existing online invoice tests still pass.

## Open Questions

1. Should drafts expire after 7 days or remain until manually cleared?
2. Should support see patient identifiers in the sync log, or only invoice IDs?
