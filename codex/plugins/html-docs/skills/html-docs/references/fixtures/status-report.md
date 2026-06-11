# Weekly Platform Report

The billing platform made forward progress on reliability work, but the release train slipped because the staging migration rehearsal exposed a missing rollback path.

## At a Glance

| Metric | This Week | Last Week | Trend |
| --- | --- | --- | --- |
| Production incidents | 1 | 3 | Better |
| Deploy lead time | 2.4 days | 2.1 days | Slightly slower |
| Error budget burn | 6% | 14% | Better |
| Open P1 bugs | 4 | 5 | Better |

## Shipped

- Queue retry telemetry now reports terminal failure reasons.
- Staging deploys publish a migration summary to the release thread.
- The invoice export job now has a bounded memory profile.

## Slipped

- The payment-ledger migration needs a reversible down migration before production.
- The customer dashboard smoke test is still flaky on slow CI runners.

## Incident Timeline

| Time | Event | Note |
| --- | --- | --- |
| 09:12 | Alerts fired | Export job exceeded memory threshold |
| 09:18 | Triage started | Failure limited to large clinics |
| 09:41 | Mitigation shipped | Batch size reduced from 500 to 100 |
| 10:20 | Follow-up filed | Add streaming export writer |

## Next Week

- Finish migration rollback rehearsal.
- Replace flaky dashboard smoke test selector.
- Decide whether export streaming belongs in the current reliability epic.
