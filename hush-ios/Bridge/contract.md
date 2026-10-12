# Hush bridge contract

The Hush iOS app talks to Bitwarden Secrets Manager only through this bridge.
The bridge wraps the pinned public crate `bitwarden = "=2.1.0"`. Bitwarden's SDK
keeps ownership of encryption and protocol behavior. The bridge only shapes
requests, enforces limits, and maps outcomes to stable codes.

The C declarations live in `include/hush_bridge.h`. Swift imports them as the
`HushBridge` module through `include/module.modulemap`.

## Functions

The ABI has four functions and nothing else.

| Function | Purpose |
| --- | --- |
| `hush_session_new(region)` | Creates a session. `0` is the US cloud, `1` is the EU cloud. Any other value returns `NULL`. |
| `hush_session_call(session, input, len)` | Runs one JSON request. Always returns a JSON envelope buffer. |
| `hush_session_drop(session)` | Destroys the session. `NULL` is a no-op. |
| `hush_buffer_free(buffer)` | Zeroizes and frees a returned buffer. A buffer with `NULL` data is a no-op. |

Region endpoints:

| Region | Identity | API |
| --- | --- | --- |
| `0` US | `https://identity.bitwarden.com` | `https://api.bitwarden.com` |
| `1` EU | `https://vault.bitwarden.eu/identity` | `https://vault.bitwarden.eu/api` |

## Caller rules

1. Swift calls every function for one session on one private serial
   `DispatchQueue`. The same queue calls `hush_session_drop`. Calls on one
   session never overlap. Overlapping calls are outside the contract.
2. The bridge copies input bytes before `hush_session_call` returns.
3. Swift frees each returned buffer exactly once with `hush_buffer_free`.
   Double free and use after free are outside the contract.
4. One call blocks the queue for at most about 30 seconds. Every network
   operation runs under a 30 second deadline.
5. A session starts locked. `login` unlocks it. A session never locks again.
   To lock, Swift drops the session and later creates a new one.

## Envelope

Success:

```json
{ "ok": true, "data": { } }
```

Failure:

```json
{ "ok": false, "error": { "code": "network" } }
```

The envelope never carries SDK error text, server bodies, or messages.

## Requests

All requests are one UTF-8 JSON object with an `op` field. Unknown fields are
rejected. IDs are lowercase or uppercase UUID strings. Output IDs are lowercase.

### login

```json
{ "op": "login", "token": "<machine access token>" }
```

Data: `{ "organizationID": "<uuid>" }`

The bridge checks the token format locally first. A malformed token returns
`invalid_token` with no network call. Login on an unlocked session returns
`invalid_request`. A failed login leaves the session locked, so Swift may try
again on the same session.

### catalog

```json
{ "op": "catalog" }
```

Data:

```json
{
  "projects": [{ "id": "<uuid>", "name": "..." }],
  "secrets": [{ "id": "<uuid>", "key": "...", "projectIDs": ["<uuid>"] }]
}
```

Catalog is metadata only. It calls the SDK project list and secret identifier
list. It never fetches secret values or notes.

### read

```json
{ "op": "read", "id": "<uuid>" }
```

Data: `{ "id", "key", "value", "note", "projectID" }`. `projectID` is a UUID
string or `null`.

### create

```json
{ "op": "create", "key": "...", "value": "...", "note": "...", "projectID": "<uuid>" }
```

Data: the same shape as `read`. A success response is the SDK round trip
result. The bridge does not add a second read.

The bridge never retries `create`, and the SDK adds no retry middleware.

## Limits

The bridge checks these limits before any network call. A breach returns
`invalid_request`. Lengths count Unicode scalar values, the same unit the SDK
validator uses.

| Field | Rule |
| --- | --- |
| whole request | 1 to 524288 bytes |
| `token` | 1 to 1024 bytes |
| `key` | 1 to 500 characters, not only whitespace |
| `value` | 1 to 25000 characters |
| `note` | 0 to 7000 characters, not only whitespace when not empty |
| `id`, `projectID` | a valid UUID |

## Error codes

| Code | Meaning |
| --- | --- |
| `invalid_request` | NULL or empty input, bad UTF-8, bad JSON, unknown op or field, bad UUID, limit breach, login on an unlocked session, or server status 400. |
| `invalid_token` | The token is malformed, or its key cannot open the login payload. |
| `authentication` | The identity server rejected the token, or the API returned 401. |
| `permission` | The API returned 403 or 404. |
| `network` | Transport failure, deadline passed, a 5xx status, or another 4xx status. Safe to retry for login, catalog, and read. |
| `locked` | catalog, read, or create before a successful login. |
| `unknown_create` | create may or may not have stored the secret: transport failure, deadline passed, a 5xx status, or a failure after the server answered. Never retry automatically. Refresh metadata and let the user decide. |
| `internal` | Unexpected SDK output, a panic, or a poisoned session. |

For `create`, these outcomes are definite and mean nothing was stored:
`invalid_request`, `authentication`, `permission`, `locked`, and `network`
for a 4xx status other than 400, 401, 403, and 404.

## Safety

- No panic crosses the C boundary. Each function runs inside `catch_unwind`.
  The crate builds with `panic = "unwind"`.
- A panic inside a session call poisons the session. Every later call returns
  `internal`. Swift drops the session and creates a new one.
- `hush_session_new` returns `NULL` on a panic.
- A panic during `hush_session_drop` leaks the session instead of unwinding.
- Returned buffers are zeroized before free. The copied request bytes, the
  token, and secret values in requests and responses are zeroized after use
  where the bridge owns them. Copies inside the SDK and the HTTP stack are
  outside bridge control.
- The bridge installs no logger and passes no SDK state file.

## Error classification

The SDK does not export its Secrets Manager error type, so the bridge cannot
match its variants by name. The bridge classifies an SDK error from its
`Debug` form: the variant name and, for server responses, the HTTP status.
The text never leaves the bridge. Mock server tests pin each class against the
locked SDK version, so a format change in an SDK update fails the tests.
