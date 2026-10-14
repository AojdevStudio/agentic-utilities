#if DEBUG
import Foundation
import os

/// Debug builds only. When UI tests set `HUSH_UI_TEST_SCENARIO`, the app runs against an in-memory
/// fake of the external boundaries: bridge, Keychain, and device authentication. Release builds
/// compile none of this. It proves view behavior only, never a live Bitwarden connection.
enum UITestHarness {
    enum Scenario: String {
        /// No saved token. Any non-empty token connects.
        case fresh
        /// A token is saved. Unlock succeeds without a prompt.
        case saved
        /// Like `saved`, but every create is stored and its reply is lost.
        case lostCreateReply
    }

    @MainActor static func store(environment: [String: String] = ProcessInfo.processInfo.environment) -> VaultStore? {
        guard let raw = environment["HUSH_UI_TEST_SCENARIO"], let scenario = Scenario(rawValue: raw) else { return nil }
        let server = FakeServer(losesCreateReply: scenario == .lostCreateReply)
        return VaultStore(
            credentials: FakeCredentials(saved: scenario != .fresh),
            deviceAuthenticator: FakeAuthenticator(),
            makeConnection: { _ in FakeConnection(server: server) }
        )
    }
}

/// Placeholder vault. Two secrets share a key and project on purpose.
private final class FakeServer: Sendable {
    let organizationID = UUID()
    private struct Row: Sendable {
        let id = UUID()
        let key: String
        let value: String
        let note: String
        let projectID: UUID
    }

    private struct State: Sendable {
        var projects: [(id: UUID, name: String)]
        var rows: [Row]
    }

    private let losesCreateReply: Bool
    private let state: OSAllocatedUnfairLock<State>

    init(losesCreateReply: Bool) {
        self.losesCreateReply = losesCreateReply
        let alpha = UUID()
        let beta = UUID()
        state = OSAllocatedUnfairLock(initialState: State(
            projects: [(alpha, "Example Alpha"), (beta, "Example Beta")],
            rows: [
                Row(key: "EXAMPLE_TOKEN", value: "placeholder-one", note: "placeholder note", projectID: alpha),
                Row(key: "EXAMPLE_TOKEN", value: "placeholder-two", note: "", projectID: alpha),
                Row(key: "EXAMPLE_TOKEN", value: "placeholder-three", note: "", projectID: beta),
                Row(key: "SAMPLE_URL", value: "placeholder-four", note: "", projectID: beta),
            ]
        ))
    }

    func catalog() -> Data {
        state.withLock { state in
            reply(ok: [
                "projects": state.projects.map { ["id": $0.id.uuidString, "name": $0.name] },
                "secrets": state.rows.map { ["id": $0.id.uuidString, "key": $0.key, "projectIDs": [$0.projectID.uuidString]] },
            ])
        }
    }

    func read(_ id: SecretID) -> Data {
        state.withLock { state in
            guard let row = state.rows.first(where: { $0.id == id.rawValue }) else { return reply(error: "permission") }
            return detail(row)
        }
    }

    func create(_ draft: NewSecret) -> Data {
        let row = Row(key: draft.key, value: draft.value, note: draft.note ?? "", projectID: draft.projectID.rawValue)
        state.withLock { $0.rows.append(row) }
        return losesCreateReply ? reply(error: "unknown_create") : detail(row)
    }

    private func detail(_ row: Row) -> Data {
        reply(ok: ["id": row.id.uuidString, "key": row.key, "value": row.value, "note": row.note, "projectID": row.projectID.uuidString])
    }
}

private func reply(ok data: Any) -> Data {
    (try? JSONSerialization.data(withJSONObject: ["ok": true, "data": data])) ?? Data()
}

private func reply(error code: String) -> Data {
    (try? JSONSerialization.data(withJSONObject: ["ok": false, "error": ["code": code]])) ?? Data()
}

private final class FakeConnection: SecretConnection {
    private let server: FakeServer
    private let closed = OSAllocatedUnfairLock(initialState: false)

    init(server: FakeServer) { self.server = server }

    func send(_ request: BridgeRequest) async throws -> Data {
        if closed.withLock({ $0 }) { throw VaultFailure.locked }
        return switch request {
        case .login: reply(ok: ["organizationID": server.organizationID.uuidString])
        case .catalog: server.catalog()
        case .read(let id): server.read(id)
        case .create(let draft): server.create(draft)
        }
    }

    func close() { closed.withLock { $0 = true } }
}

private final class FakeCredentials: CredentialStore {
    private let saved: OSAllocatedUnfairLock<Credential?>

    init(saved: Bool) {
        self.saved = OSAllocatedUnfairLock(initialState: saved ? Credential(token: "ui-test-placeholder", region: .us) : nil)
    }

    func hasCredential() async -> Bool { saved.withLock { $0 != nil } }

    func load() async throws(VaultFailure) -> Credential {
        guard let credential = saved.withLock({ $0 }) else { throw .notConfigured }
        return credential
    }

    func save(_ credential: Credential) async throws(VaultFailure) { saved.withLock { $0 = credential } }

    func delete() async throws(VaultFailure) { saved.withLock { $0 = nil } }
}

private struct FakeAuthenticator: DeviceAuthenticator {
    func authenticate() async throws(VaultFailure) {}
}
#endif
