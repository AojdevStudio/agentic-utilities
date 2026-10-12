import Foundation
import os
import Testing
import UIKit
@testable import Hush

@MainActor @Suite(.serialized, .timeLimit(.minutes(1))) struct AppModelTests {
    @Test(arguments: [false, true])
    func automaticSessionCleanupClearsOnlyOurClipboard(foreignCopy: Bool) async throws {
        let pasteboard = UIPasteboard.general
        let previous = pasteboard.items
        defer { pasteboard.items = previous }
        let poisoned = OSAllocatedUnfairLock(initialState: false)
        let backend = FakeBackend { request in
            poisoned.withLock { $0 } ? Reply.error("internal") : standardReply(request)
        }
        let store = VaultStore(credentials: FakeCredentialStore(),
                               deviceAuthenticator: FakeDeviceAuthenticator(), makeConnection: backend.factory)
        let model = AppModel(store: store)
        try await store.connect(token: "0.test", region: .us)
        model.selection = Fixture.stripe.id
        await model.loadDetail(Fixture.stripe.id)
        model.copyValue()
        #expect(pasteboard.string == Fixture.stripeDetail.value)
        if foreignCopy { pasteboard.string = "someone-elses-copy" }

        poisoned.withLock { $0 = true }
        await model.refresh()
        #expect(store.phase == .locked)
        // RootView calls this shared cleanup when the store locks after a poisoned reply.
        model.clearVault()

        if foreignCopy {
            #expect(pasteboard.string == "someone-elses-copy")
        } else {
            #expect(!pasteboard.hasStrings)
        }
    }

    @Test func earlierDetailReplyDoesNotReplaceTheNewSelection() async throws {
        let gate = Gate()
        let later = SecretDetail(id: Fixture.infraDatabase.id, key: Fixture.infraDatabase.key,
                                 value: "later-value", note: "", projectID: Fixture.infra.id)
        let backend = FakeBackend { request in
            if case .read(let id) = request {
                if id == Fixture.stripe.id {
                    await gate.wait()
                    return Reply.detail(Fixture.stripeDetail)
                }
                if id == later.id { return Reply.detail(later) }
            }
            return standardReply(request)
        }
        let store = VaultStore(credentials: FakeCredentialStore(),
                               deviceAuthenticator: FakeDeviceAuthenticator(), makeConnection: backend.factory)
        let model = AppModel(store: store)
        try await store.connect(token: "0.test", region: .us)
        model.selection = Fixture.stripe.id
        let earlier = Task { await model.loadDetail(Fixture.stripe.id) }
        _ = await backend.requests.next { request in
            if case .read(let id) = request { id == Fixture.stripe.id } else { false }
        }
        model.selection = later.id
        await model.loadDetail(later.id)
        #expect(model.detail == .loaded(later))

        await gate.open()
        await earlier.value
        #expect(model.selection == later.id)
        #expect(model.detail == .loaded(later))
    }
}
