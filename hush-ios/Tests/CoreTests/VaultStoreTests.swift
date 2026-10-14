import Foundation
import Testing
@testable import Hush

@MainActor @Suite(.timeLimit(.minutes(1))) struct VaultStoreTests {
    let saved = Credential(token: "0.saved", region: .us)

    func makeStore(
        _ backend: FakeBackend,
        credentials: FakeCredentialStore = FakeCredentialStore(),
        authenticator: FakeDeviceAuthenticator = FakeDeviceAuthenticator()
    ) -> VaultStore {
        VaultStore(credentials: credentials, deviceAuthenticator: authenticator, makeConnection: backend.factory)
    }

    func draft() throws -> NewSecret {
        try NewSecret(key: "NEW_KEY", value: "new value", note: nil, projectID: Fixture.infra.id, in: Fixture.catalog)
    }

    // MARK: Connect

    @Test func connectSavesCredentialAfterLoginCatalogAndDeviceAuth() async throws {
        let backend = FakeBackend()
        let credentials = FakeCredentialStore()
        let authenticator = FakeDeviceAuthenticator()
        let store = makeStore(backend, credentials: credentials, authenticator: authenticator)

        try await store.connect(token: "0.new", region: .eu)

        #expect(store.phase == .unlocked(Fixture.catalog))
        #expect(credentials.saved == Credential(token: "0.new", region: .eu))
        #expect(backend.regions == [.eu])
        #expect(authenticator.prompts == 1)
    }

    @Test func connectWithRejectedTokenSavesNothing() async {
        let backend = FakeBackend { $0.isCatalog ? Reply.catalog(Fixture.catalog) : Reply.error("invalid_token") }
        let credentials = FakeCredentialStore()
        let authenticator = FakeDeviceAuthenticator()
        let store = makeStore(backend, credentials: credentials, authenticator: authenticator)

        await #expect(throws: VaultFailure.invalidToken) { try await store.connect(token: "bad", region: .us) }

        #expect(store.phase == .disconnected)
        #expect(credentials.saved == nil)
        #expect(authenticator.prompts == 0)
        #expect(backend.connections.allSatisfy { $0.isClosed })
    }

    @Test func cancelledDeviceAuthKeepsTheOldCredential() async {
        let backend = FakeBackend()
        let credentials = FakeCredentialStore(saved: saved)
        let store = makeStore(
            backend, credentials: credentials,
            authenticator: FakeDeviceAuthenticator(failure: .deviceAuthenticationCancelled)
        )
        await store.restore()

        await #expect(throws: VaultFailure.deviceAuthenticationCancelled) {
            try await store.connect(token: "0.new", region: .eu)
        }

        #expect(store.phase == .locked)
        #expect(credentials.saved == saved)
        #expect(backend.connections.allSatisfy { $0.isClosed })
    }

    // MARK: Stale results

    @Test func lockDuringLoginRejectsTheLateLogin() async {
        let gate = Gate()
        let backend = FakeBackend { request in
            if case .login = request { await gate.wait() }
            return standardReply(request)
        }
        let credentials = FakeCredentialStore()
        let authenticator = FakeDeviceAuthenticator()
        let store = makeStore(backend, credentials: credentials, authenticator: authenticator)

        let connecting = Task { try await store.connect(token: "0.new", region: .us) }
        _ = await backend.requests.next()
        store.lock()
        #expect(store.phase == .disconnected)
        await gate.open()

        await #expect(throws: VaultFailure.locked) { try await connecting.value }
        #expect(store.phase == .disconnected)
        #expect(credentials.saved == nil)
        #expect(authenticator.prompts == 0)
        #expect(!backend.sent.contains { $0.isCatalog })
    }

    @Test func lockDuringUnlockCatalogStaysLocked() async {
        let gate = Gate()
        let backend = FakeBackend { request in
            if request.isCatalog { await gate.wait() }
            return standardReply(request)
        }
        let store = makeStore(backend, credentials: FakeCredentialStore(saved: saved))
        await store.restore()

        let unlocking = Task { try await store.unlock() }
        _ = await backend.requests.next { $0.isCatalog }
        store.lock()
        await gate.open()

        await #expect(throws: VaultFailure.locked) { try await unlocking.value }
        #expect(store.phase == .locked)
        #expect(backend.connections.allSatisfy { $0.isClosed })
    }

    @Test func lockDuringReadDropsTheLateValue() async throws {
        let gate = Gate()
        let backend = FakeBackend { request in
            if case .read = request { await gate.wait() }
            return standardReply(request)
        }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)

        let reading = Task { try await store.read(Fixture.stripe.id) }
        _ = await backend.requests.next { if case .read = $0 { true } else { false } }
        store.lock()
        await gate.open()

        await #expect(throws: VaultFailure.locked) { try await reading.value }
        #expect(store.phase == .locked)
    }

    @Test func readReturnsTheDetail() async throws {
        let store = makeStore(FakeBackend())
        try await store.connect(token: "0.new", region: .us)
        #expect(try await store.read(Fixture.stripe.id) == Fixture.stripeDetail)
    }

    // MARK: Unlock and forget

    @Test func cancelledUnlockPromptKeepsTheCredential() async {
        let backend = FakeBackend()
        let credentials = FakeCredentialStore(saved: saved, loadFailure: .deviceAuthenticationCancelled)
        let store = makeStore(backend, credentials: credentials)
        await store.restore()

        await #expect(throws: VaultFailure.deviceAuthenticationCancelled) { try await store.unlock() }

        #expect(store.phase == .locked)
        #expect(credentials.saved == saved)
        #expect(backend.connections.isEmpty)
    }

    @Test func revokedTokenStaysLockedAndForgetWorksOffline() async throws {
        let backend = FakeBackend { _ in Reply.error("invalid_token") }
        let credentials = FakeCredentialStore(saved: saved)
        let store = makeStore(backend, credentials: credentials)
        await store.restore()

        await #expect(throws: VaultFailure.invalidToken) { try await store.unlock() }
        #expect(store.phase == .locked)

        let sentBeforeForget = backend.sent.count
        try await store.forget()
        #expect(store.phase == .disconnected)
        #expect(credentials.saved == nil)
        #expect(backend.sent.count == sentBeforeForget)
    }

    // MARK: Malformed replies

    @Test(arguments: [
        Data("not json".utf8),
        Reply.json(["ok": true]),
        Reply.json(["ok": false]),
        Reply.json(["ok": true, "data": ["id": "nope", "key": "K", "value": "v", "note": ""]]),
        Reply.detail(SecretDetail(id: SecretID(UUID()), key: "OTHER", value: "v", note: "", projectID: nil)),
        Data(),
    ])
    func malformedReadReplyIsGenericAndLocks(reply: Data) async throws {
        let backend = FakeBackend { request in
            if case .read = request { return reply }
            return standardReply(request)
        }
        let store = makeStore(backend, credentials: FakeCredentialStore())
        try await store.connect(token: "0.new", region: .us)

        await #expect(throws: VaultFailure.generic) { try await store.read(Fixture.stripe.id) }
        #expect(store.phase == .locked)
    }

    @Test func unknownErrorCodeIsGenericAndKeepsTheSession() async throws {
        let backend = FakeBackend { request in
            if case .read = request { return Reply.error("teapot") }
            return standardReply(request)
        }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)

        await #expect(throws: VaultFailure.generic) { try await store.read(Fixture.stripe.id) }
        #expect(store.phase == .unlocked(Fixture.catalog))
    }

    // MARK: Create

    @Test func lostCreateIsSentOnceAndBlocksRepeatUntilReviewed() async throws {
        let backend = FakeBackend { $0.isCreate ? Reply.error("unknown_create") : standardReply($0) }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)
        let draft = try draft()

        guard case .unconfirmed(let attempt) = await store.create(draft) else {
            Issue.record("expected unconfirmed")
            return
        }
        #expect(attempt.key == "NEW_KEY")
        #expect(attempt.projectID == Fixture.infra.id)
        #expect(attempt.priorIDs == Set(Fixture.catalog.secrets.map(\.id)))
        #expect(store.pendingCreate == attempt)

        #expect(await store.create(draft) == .rejected(.createPending))
        #expect(backend.createCount == 1)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }

        try await store.refresh()
        #expect(store.canAcknowledgePendingCreate)
        try store.acknowledgePendingCreate()
        #expect(store.pendingCreate == nil)

        _ = await store.create(draft)
        #expect(backend.createCount == 2)
    }

    @Test func differentOrganizationCannotReviewPendingCreate() async throws {
        let otherOrganizationID = UUID()
        let backend = FakeBackend { request in
            if case .login(let token) = request, token == "0.other" {
                return Reply.login(organizationID: otherOrganizationID)
            }
            return request.isCreate ? Reply.error("unknown_create") : standardReply(request)
        }
        let store = makeStore(backend)
        try await store.connect(token: "0.original", region: .us)
        _ = await store.create(try draft())
        let attempt = try #require(store.pendingCreate)
        try await store.refresh()
        try await store.forget()

        try await store.connect(token: "0.other", region: .us)
        #expect(!store.canAcknowledgePendingCreate)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }
        try await store.refresh()
        #expect(!store.canAcknowledgePendingCreate)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }
        #expect(store.pendingCreate == attempt)
        #expect(backend.createCount == 1)
    }

    @Test func differentRegionCannotReviewPendingCreate() async throws {
        let backend = FakeBackend { $0.isCreate ? Reply.error("unknown_create") : standardReply($0) }
        let store = makeStore(backend)
        try await store.connect(token: "0.original", region: .us)
        _ = await store.create(try draft())
        let attempt = try #require(store.pendingCreate)
        store.lock()

        try await store.connect(token: "0.other-region", region: .eu)
        try await store.refresh()
        #expect(!store.canAcknowledgePendingCreate)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }
        #expect(store.pendingCreate == attempt)
        #expect(backend.createCount == 1)
    }

    @Test func sameVaultTokenRotationCanReviewPendingCreateAfterNewRefresh() async throws {
        let backend = FakeBackend { $0.isCreate ? Reply.error("unknown_create") : standardReply($0) }
        let store = makeStore(backend)
        try await store.connect(token: "0.original", region: .us)
        _ = await store.create(try draft())
        let attempt = try #require(store.pendingCreate)
        try await store.refresh()
        store.lock()
        #expect(!store.canAcknowledgePendingCreate)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }

        try await store.connect(token: "0.rotated", region: .us)
        #expect(!store.canAcknowledgePendingCreate)
        #expect(store.pendingCreate == attempt)
        try await store.refresh()
        #expect(store.canAcknowledgePendingCreate)
        try store.acknowledgePendingCreate()
        #expect(store.pendingCreate == nil)
        #expect(backend.createCount == 1)
    }

    @Test func lockDuringCreateIsUnconfirmedEvenWhenTheServerSucceeds() async throws {
        let gate = Gate()
        let backend = FakeBackend { request in
            if request.isCreate { await gate.wait() }
            return standardReply(request)
        }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)

        let creating = Task { await store.create(try draft()) }
        _ = await backend.requests.next { $0.isCreate }
        #expect(!store.pendingCreateSettled)
        try await store.refresh()
        #expect(!store.canAcknowledgePendingCreate)
        #expect(throws: VaultFailure.createPending) { try store.acknowledgePendingCreate() }
        store.lock()
        await gate.open()

        guard case .unconfirmed = try await creating.value else {
            Issue.record("expected unconfirmed")
            return
        }
        #expect(store.phase == .locked)
        #expect(store.pendingCreate != nil)
        #expect(store.pendingCreateSettled)
        #expect(backend.createCount == 1)
    }

    @Test func malformedCreateReplyIsUnconfirmed() async throws {
        let backend = FakeBackend { $0.isCreate ? Reply.json(["ok": true]) : standardReply($0) }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)

        guard case .unconfirmed = await store.create(try draft()) else {
            Issue.record("expected unconfirmed")
            return
        }
        #expect(store.pendingCreate != nil)
    }

    @Test func confirmedCreateAddsTheSummaryAndClearsTheAttempt() async throws {
        let store = makeStore(FakeBackend())
        try await store.connect(token: "0.new", region: .us)

        guard case .created(let detail) = await store.create(try draft()) else {
            Issue.record("expected created")
            return
        }
        #expect(store.pendingCreate == nil)
        guard case .unlocked(let catalog) = store.phase else {
            Issue.record("expected unlocked")
            return
        }
        #expect(catalog.secrets.contains(detail.summary))
    }

    @Test func rejectedCreateClearsTheAttempt() async throws {
        let backend = FakeBackend { $0.isCreate ? Reply.error("permission") : standardReply($0) }
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)

        #expect(await store.create(try draft()) == .rejected(.permission))
        #expect(store.pendingCreate == nil)
    }

    @Test func createWhileLockedSendsNothing() async throws {
        let backend = FakeBackend()
        let store = makeStore(backend)
        try await store.connect(token: "0.new", region: .us)
        store.lock()

        #expect(await store.create(try draft()) == .rejected(.locked))
        #expect(backend.createCount == 0)
    }
}
