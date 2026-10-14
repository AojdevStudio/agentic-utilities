import Foundation
import os
@testable import Hush

nonisolated enum Fixture {
    static let organizationID = UUID()
    static let infra = Project(id: ProjectID(UUID()), name: "Infra")
    static let cafe = Project(id: ProjectID(UUID()), name: "Café App")
    static let infraDatabase = SecretSummary(id: SecretID(UUID()), key: "DATABASE_URL", projectIDs: [infra.id])
    static let cafeDatabase = SecretSummary(id: SecretID(UUID()), key: "DATABASE_URL", projectIDs: [cafe.id])
    static let stripe = SecretSummary(id: SecretID(UUID()), key: "STRIPE_KEY", projectIDs: [cafe.id])
    static let catalog = Catalog(projects: [infra, cafe], secrets: [stripe, infraDatabase, cafeDatabase])

    static let stripeDetail = SecretDetail(
        id: stripe.id, key: stripe.key, value: "sk_test_value", note: "rotate yearly", projectID: cafe.id
    )
}

nonisolated enum Reply {
    static func ok(_ data: Any) -> Data { json(["ok": true, "data": data]) }
    static func error(_ code: String) -> Data { json(["ok": false, "error": ["code": code]]) }
    static func login(organizationID: UUID = Fixture.organizationID) -> Data {
        ok(["organizationID": organizationID.uuidString])
    }

    static func catalog(_ catalog: Catalog) -> Data {
        ok([
            "projects": catalog.projects.map { ["id": $0.id.rawValue.uuidString, "name": $0.name] },
            "secrets": catalog.secrets.map {
                ["id": $0.id.rawValue.uuidString, "key": $0.key, "projectIDs": $0.projectIDs.map(\.rawValue.uuidString)]
            },
        ])
    }

    static func detail(_ detail: SecretDetail) -> Data {
        ok([
            "id": detail.id.rawValue.uuidString,
            "key": detail.key,
            "value": detail.value,
            "note": detail.note,
            "projectID": detail.projectID.map { $0.rawValue.uuidString as Any } ?? NSNull(),
        ])
    }

    static func json(_ object: Any) -> Data {
        try! JSONSerialization.data(withJSONObject: object)
    }
}

typealias Responder = @Sendable (BridgeRequest) async throws -> Data

nonisolated extension BridgeRequest {
    var isCreate: Bool { if case .create = self { true } else { false } }
    var isCatalog: Bool { if case .catalog = self { true } else { false } }
}

/// Serves `Fixture.catalog` and echoes creates as confirmed.
nonisolated func standardReply(_ request: BridgeRequest) -> Data {
    switch request {
    case .login: Reply.login()
    case .catalog: Reply.catalog(Fixture.catalog)
    case .read(let id): id == Fixture.stripe.id ? Reply.detail(Fixture.stripeDetail) : Reply.error("invalid_request")
    case .create(let draft):
        Reply.detail(SecretDetail(
            id: SecretID(UUID()), key: draft.key, value: draft.value, note: draft.note ?? "", projectID: draft.projectID
        ))
    }
}

/// Hands out fake connections and records every request across them, in order.
nonisolated final class FakeBackend: Sendable {
    private struct State: Sendable {
        var regions: [Region] = []
        var connections: [FakeConnection] = []
    }

    let requests: AsyncStream<BridgeRequest>
    private let sink: AsyncStream<BridgeRequest>.Continuation
    private let responder: Responder
    private let state = OSAllocatedUnfairLock(initialState: State())

    init(responder: @escaping Responder = { standardReply($0) }) {
        (requests, sink) = AsyncStream.makeStream()
        self.responder = responder
    }

    var factory: ConnectionFactory {
        { [self] region in
            let connection = FakeConnection(responder: responder, sink: sink)
            state.withLock {
                $0.regions.append(region)
                $0.connections.append(connection)
            }
            return connection
        }
    }

    var regions: [Region] { state.withLock { $0.regions } }
    var connections: [FakeConnection] { state.withLock { $0.connections } }
    var sent: [BridgeRequest] { connections.flatMap(\.sent) }
    var createCount: Int { sent.filter(\.isCreate).count }
}

nonisolated final class FakeConnection: SecretConnection {
    private struct State: Sendable {
        var sent: [BridgeRequest] = []
        var closed = false
    }

    private let responder: Responder
    private let sink: AsyncStream<BridgeRequest>.Continuation
    private let state = OSAllocatedUnfairLock(initialState: State())

    init(responder: @escaping Responder, sink: AsyncStream<BridgeRequest>.Continuation) {
        self.responder = responder
        self.sink = sink
    }

    func send(_ request: BridgeRequest) async throws -> Data {
        state.withLock { $0.sent.append(request) }
        sink.yield(request)
        return try await responder(request)
    }

    func close() { state.withLock { $0.closed = true } }

    var sent: [BridgeRequest] { state.withLock { $0.sent } }
    var isClosed: Bool { state.withLock { $0.closed } }
}

nonisolated final class FakeCredentialStore: CredentialStore {
    private struct State: Sendable {
        var saved: Credential?
        var loadFailure: VaultFailure?
    }

    private let state: OSAllocatedUnfairLock<State>

    init(saved: Credential? = nil, loadFailure: VaultFailure? = nil) {
        state = OSAllocatedUnfairLock(initialState: State(saved: saved, loadFailure: loadFailure))
    }

    var saved: Credential? { state.withLock { $0.saved } }

    func hasCredential() async -> Bool { saved != nil }

    func load() async throws(VaultFailure) -> Credential {
        let current = state.withLock { $0 }
        if let failure = current.loadFailure { throw failure }
        guard let saved = current.saved else { throw .notConfigured }
        return saved
    }

    func save(_ credential: Credential) async throws(VaultFailure) {
        state.withLock { $0.saved = credential }
    }

    func delete() async throws(VaultFailure) {
        state.withLock { $0.saved = nil }
    }
}

nonisolated final class FakeDeviceAuthenticator: DeviceAuthenticator {
    private let failure: VaultFailure?
    private let count = OSAllocatedUnfairLock(initialState: 0)

    init(failure: VaultFailure? = nil) { self.failure = failure }

    var prompts: Int { count.withLock { $0 } }

    func authenticate() async throws(VaultFailure) {
        count.withLock { $0 += 1 }
        if let failure { throw failure }
    }
}

/// Holds a fake reply until the test opens it.
actor Gate {
    private var waiters: [CheckedContinuation<Void, Never>] = []
    private var isOpen = false

    func wait() async {
        if isOpen { return }
        await withCheckedContinuation { waiters.append($0) }
    }

    func open() {
        isOpen = true
        waiters.forEach { $0.resume() }
        waiters.removeAll()
    }
}

nonisolated extension AsyncStream where Element: Sendable {
    /// Waits for the next element that passes `match`, skipping the rest.
    func next(where match: @Sendable (Element) -> Bool = { _ in true }) async -> Element? {
        var iterator = makeAsyncIterator()
        while let element = await iterator.next() {
            if match(element) { return element }
        }
        return nil
    }
}
