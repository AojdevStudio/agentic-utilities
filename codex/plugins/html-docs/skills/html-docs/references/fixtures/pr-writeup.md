# PR #128: Move Invoice Email Delivery Onto a Queue

This PR removes synchronous SMTP calls from checkout completion and moves invoice email delivery to the existing background queue.

## TL;DR

- Checkout completion now records an `invoice.email_requested` event.
- A queue worker sends the email and writes the delivery result.
- Failed email delivery no longer blocks checkout success.

## Before and After

| Area | Before | After |
| --- | --- | --- |
| Checkout latency | Waited on SMTP response | Returns after invoice commit |
| Retry behavior | Manual support resend | Queue retries with backoff |
| Observability | Checkout logs only | Delivery table plus worker logs |

## File Tour

### `apps/api/src/checkout/complete.ts`

Creates the invoice and emits the email request event inside the same transaction.

### `apps/workers/src/invoice-email-worker.ts`

Consumes queued email requests, sends the invoice, and records success or terminal failure.

### `apps/api/test/checkout-email.test.ts`

Adds regression coverage for successful checkout when SMTP is unavailable.

## Review Focus

- Confirm the event is committed atomically with the invoice.
- Check whether queue retry settings match existing notification workers.
- Verify no customer-facing copy changed.

## Rollback

Disable the `invoice-email-worker` consumer and re-enable the previous synchronous sender behind the existing feature flag.
