# Hush Core API

This is the contract between the Core files and the app views. Everything here lives in the single app module `Hush`. Tests use `@testable import Hush`. Nothing is `public`.

## Files

| File | What it owns |
|---|---|
| `Domain.swift` | IDs, `Region`, `VaultIdentity`, `Project`, `SecretSummary`, `Catalog`, `SecretDetail`, `NewSecret`, `CreateAttempt`, `CreateOutcome`, `VaultFailure`, `SessionPhase`. Pure values. |
| `BridgeWire.swift` | `BridgeRequest`, the JSON request encoder, and the reply decoder. No C. |
| `SecretConnection.swift` | The `SecretConnection` protocol. Tests use a fake. |
| `SDKConnection.swift` | Production `SecretConnection` over the `HushBridge` C module, and `VaultStore.live()`. The only file that imports `HushBridge`. |
| `VaultStore.swift` | `@MainActor @Observable` session state. The only object views call for vault work. |
| `DeviceSecurity.swift` | `Credential`, `CredentialStore`, `KeychainCredentialStore`, `DeviceAuthenticator`, `LocalDeviceAuthenticator`. |
| `Clipboard.swift` | `@MainActor` local, expiring pasteboard copy. |

## Values

```swift
struct SecretID: Hashable, Codable, Sendable { let rawValue: UUID }
struct ProjectID: Hashable, Codable, Sendable { let rawValue: UUID }

enum Region: String, Codable, CaseIterable, Sendable { case us, eu }
// Not a secret. The app may keep the last picked region in @AppStorage.
struct VaultIdentity: Hashable, Sendable { let region: Region; let organizationID: UUID }

struct Project: Identifiable, Hashable, Sendable { let id: ProjectID; let name: String }
struct SecretSummary: Identifiable, Hashable, Sendable { let id: SecretID; let key: String; let projectIDs: [ProjectID] }

struct Catalog: Hashable, Sendable {
    init(projects: [Project], secrets: [SecretSummary])   // sorts both by name, then by ID
    let projects: [Project]
    let secrets: [SecretSummary]
    func project(_ id: ProjectID) -> Project?
    func matching(query: String, project: ProjectID?) -> [SecretSummary]
}
```

`matching` is pure. It trims the query. An empty query keeps every row. A row matches when its key, or the name of one of its projects, contains the query. Case and diacritics are ignored. `project` keeps only rows in that project. Two secrets with the same key stay two rows with two IDs.

```swift
struct SecretDetail: Identifiable, Hashable, Sendable {
    let id: SecretID; let key: String; let value: String; let note: String; let projectID: ProjectID?
}
```

`description`, `debugDescription`, and the mirror redact `value`. The store never keeps a detail. The view that asked for it owns it and must drop it on lock and on background.

```swift
struct NewSecret: Sendable {
    init(key: String, value: String, note: String?, projectID: ProjectID?, in catalog: Catalog) throws(NewSecret.Invalid)
    let key: String       // trimmed
    let value: String     // as typed, must not be empty
    let note: String?     // as typed
    let projectID: ProjectID
    enum Invalid: Error { case emptyKey, emptyValue, missingProject, unknownProject }
}
```

Errors are checked in this order: key, value, project. Use `Invalid` to mark form fields.

## Session

```swift
enum SessionPhase: Hashable, Sendable {
    case disconnected        // no saved credential
    case locked              // a credential is saved, no live session
    case unlocking           // connect or unlock is running
    case unlocked(Catalog)
}

@MainActor @Observable final class VaultStore {
    static func live() -> VaultStore           // production wiring
    private(set) var phase: SessionPhase       // starts .disconnected
    private(set) var pendingCreate: CreateAttempt?
    private(set) var canAcknowledgePendingCreate: Bool
    private(set) var vaultIdentity: VaultIdentity?
    private(set) var pendingCreateSettled: Bool

    func restore() async                       // at launch: .disconnected -> .locked if a credential is saved. No prompt.
    func connect(token: String, region: Region) async throws(VaultFailure)
    func unlock() async throws(VaultFailure)
    func refresh() async throws(VaultFailure)
    func read(_ id: SecretID) async throws(VaultFailure) -> SecretDetail
    func create(_ draft: NewSecret) async -> CreateOutcome
    func acknowledgePendingCreate() throws(VaultFailure)
    func lock()                                // synchronous
    func forget() async throws(VaultFailure)
}
```

### What each call does

- `connect`: signs in with the token, loads the catalog, asks for device authentication, then saves token and region as one Keychain record. The record is saved only when all four steps pass. Any failure leaves the old saved credential as it was. Callable from `.disconnected` or `.locked` (replace a token). On success the phase is `.unlocked`.
- `unlock`: reads the saved credential (this shows the device authentication prompt), signs in, and loads the catalog. A cancelled prompt throws `.deviceAuthenticationCancelled` and keeps the credential. A rejected token throws `.invalidToken` and keeps the credential. Offer `forget()` then `connect` to replace it.
- `refresh`: reloads the catalog. Needs `.unlocked`.
- `read`: fetches one secret with its value. Nothing is cached.
- `create`: see below.
- `lock`: sets the phase at once (`.locked`, or `.disconnected` when nothing is saved) and drops the live session. Calls still in flight cannot unlock or repopulate the store. Reads drop late values and throw `.locked`. Call it when the scene goes to the background. A create in flight ends `.unconfirmed` with its metadata attempt.
- `forget`: drops the session and deletes the saved credential without contacting the server. Ends in `.disconnected`. It keeps any pending create attempt.

### Create

```swift
struct CreateAttempt: Identifiable, Hashable, Sendable {
    let id: UUID; let key: String; let projectID: ProjectID; let priorIDs: Set<SecretID>; let startedAt: Date; let vaultIdentity: VaultIdentity
    func candidates(in catalog: Catalog) -> [SecretSummary]   // new IDs with the same key and project
}

enum CreateOutcome: Hashable, Sendable {
    case created(SecretDetail)       // the store also adds the summary to the catalog
    case rejected(VaultFailure)      // nothing was written. Keep the draft.
    case unconfirmed(CreateAttempt)  // the server may or may not have it
}
```

Rules:

1. One create is sent at most once. There is no automatic retry.
2. While `pendingCreate` is set, `create` returns `.rejected(.createPending)` without sending.
3. While the create is in flight, `pendingCreateSettled` is false. Show that Hush is waiting for a reply. A refresh that began before it settled cannot enable acknowledgment.
4. After `.unconfirmed`, the user refreshes the original vault. Only a successful `refresh()` with the attempt's region and organization sets `canAcknowledgePendingCreate`. Show `attempt.candidates(in: catalog)` as evidence, not proof.
5. `acknowledgePendingCreate()` clears the attempt only while the original vault is unlocked and refreshed. Otherwise it throws `.createPending`. Locking or changing the session resets review permission. A rotated token for the same region and organization can review the attempt after a new refresh.
6. The attempt holds only key, project, prior IDs, start time, and vault identity, in memory. It survives lock and forget. It never holds the token, value, or note. `vaultIdentity` is nil outside a live signed-in session.

## Failures

```swift
enum VaultFailure: Error, Hashable, Sendable {
    case invalidInput, invalidToken, authentication, permission, network, locked,
         notConfigured, deviceAuthenticationCancelled, deviceAuthenticationUnavailable,
         deviceAuthenticationFailed, createPending, generic
}
```

Raw bridge, SDK, and Keychain errors never reach the views. A malformed bridge reply is `.generic`. A bridge `locked` or `internal` code, or a malformed reply, also locks the store because the session may be poisoned.

## Clipboard

```swift
@MainActor final class Clipboard {
    init(pasteboard: UIPasteboard = .general, lifetime: Duration = .seconds(30))
    func copy(_ value: String)   // localOnly: true, expirationDate: now + lifetime
    func clearIfOwned()          // clears only if nothing else was copied since
}
```

`copy` also schedules `clearIfOwned` after the lifetime. Call `clearIfOwned` on lock if you want the value gone before the system expiry. Call `release` when the app leaves the foreground, so another app can paste the value until the system expiry.

## Bridge wire format (Core side)

Core sends one UTF-8 JSON object per `hush_session_call`:

| Request | JSON | `data` on success |
|---|---|---|
| login | `{"op":"login","token":"…"}` | `{"organizationID":"uuid"}` |
| catalog | `{"op":"catalog"}` | `{"projects":[{"id","name"}],"secrets":[{"id","key","projectIDs":[…]}]}` |
| read | `{"op":"read","id":"uuid"}` | `{"id","key","value","note","projectID"}` (`projectID` may be null) |
| create | `{"op":"create","key","value","note","projectID"}` | same as read |

Replies: `{"ok":true,"data":…}` or `{"ok":false,"error":{"code":"…"}}`. Codes: `invalid_request`, `invalid_token`, `authentication`, `permission`, `network`, `locked`, `unknown_create`, `internal`. This matches `Bridge/contract.md`. For create, only `invalid_request`, `authentication`, `permission`, `locked`, and `network` count as rejected. Every other create failure is unconfirmed.

The bridge also rejects with `invalid_request` a key over 500 characters, a value over 25000, a note over 7000, or a note that is only whitespace. `NewSecret` does not check these, so they arrive as `.rejected(.invalidInput)`.

## Build needs

- The `HushBridge` XCFramework headers must ship a `module.modulemap` that declares `module HushBridge`, so `import HushBridge` works in `SDKConnection.swift`.
- Link frameworks: Security, LocalAuthentication, UIKit, Observation.
- iOS 17 or later (`OSAllocatedUnfairLock`, `@Observable`).
- Works with Swift 6 strict concurrency, with default actor isolation set to either `nonisolated` or `MainActor`.
