import Foundation
import LocalAuthentication
import Security
import Testing
import UIKit
@testable import Hush

@MainActor @Suite(.timeLimit(.minutes(1))) struct ClipboardTests {
    @Test func copyIsLocalOnlyAndExpires() {
        let now = Date(timeIntervalSinceReferenceDate: 1_000)
        let options = Clipboard.options(now: now, lifetime: .seconds(30))
        #expect(options[.localOnly] as? Bool == true)
        #expect(options[.expirationDate] as? Date == now.addingTimeInterval(30))
    }

    @Test func copiedValueIsGoneAfterTheLifetime() async throws {
        let pasteboard = UIPasteboard.withUniqueName()
        let clipboard = Clipboard(pasteboard: pasteboard, lifetime: .seconds(1))

        clipboard.copy("s3cret")
        #expect(pasteboard.string == "s3cret")

        try await Task.sleep(for: .milliseconds(1_500))
        #expect(!pasteboard.hasStrings)
    }

    @Test func clearLeavesSomeoneElsesCopy() {
        let pasteboard = UIPasteboard.withUniqueName()
        let clipboard = Clipboard(pasteboard: pasteboard)

        clipboard.copy("s3cret")
        pasteboard.string = "theirs"
        clipboard.clearIfOwned()

        #expect(pasteboard.string == "theirs")
    }

    @Test func clearRemovesOurCopy() {
        let pasteboard = UIPasteboard.withUniqueName()
        let clipboard = Clipboard(pasteboard: pasteboard)

        clipboard.copy("s3cret")
        clipboard.clearIfOwned()

        #expect(!pasteboard.hasStrings)
    }
}

@Suite(.serialized, .timeLimit(.minutes(1))) struct KeychainCredentialStoreTests {
    @Test func savedRecordIsDeviceOnlyPasscodeGatedAndNotSynced() async throws {
        let store = KeychainCredentialStore(service: "HushTests-\(UUID().uuidString)")
        try await store.save(Credential(token: "0.test", region: .eu))
        #expect(await store.hasCredential())
        try await store.save(Credential(token: "0.replacement", region: .us))

        let attributes = try #require(storedAttributes(service: store.service, account: store.account))
        #expect(attributes[kSecAttrAccessible as String] as? String == kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly as String)
        #expect((attributes[kSecAttrSynchronizable as String] as? NSNumber)?.boolValue != true)
        #expect(attributes[kSecAttrAccessControl as String] != nil)

        try await store.delete()
        #expect(!(await store.hasCredential()))
    }

    @Test(arguments: [
        (errSecUserCanceled, VaultFailure.deviceAuthenticationCancelled),
        (errSecAuthFailed, VaultFailure.deviceAuthenticationFailed),
        (errSecInteractionNotAllowed, VaultFailure.deviceAuthenticationUnavailable),
        (errSecParam, VaultFailure.generic),
    ])
    func failedReplacementKeepsTheSavedCredential(status: OSStatus, failure: VaultFailure) async throws {
        let store = KeychainCredentialStore(
            service: "HushTests-\(UUID().uuidString)",
            addItem: { _ in
                Issue.record("A failed replacement must not add a record.")
                return status
            },
            updateItem: { _, _ in status }
        )
        let original = Credential(token: "0.original", region: .us)
        // This readable real Keychain fixture avoids a device prompt in this failure-path test.
        // The production policy is checked separately by savedRecordIsDeviceOnlyPasscodeGatedAndNotSynced.
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: store.service,
            kSecAttrAccount as String: store.account,
            kSecUseDataProtectionKeychain as String: true,
        ]
        var attributes = query
        attributes[kSecValueData as String] = try JSONEncoder().encode(original)
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        attributes[kSecAttrSynchronizable as String] = kCFBooleanFalse
        try #require(SecItemAdd(attributes as CFDictionary, nil) == errSecSuccess)
        defer { SecItemDelete(query as CFDictionary) }

        await #expect(throws: failure) {
            try await store.save(Credential(token: "0.replacement", region: .eu))
        }

        #expect(await store.hasCredential())
        #expect(try await store.load() == original)
    }

    @Test func deleteSucceedsWhenNothingIsSaved() async throws {
        let store = KeychainCredentialStore(service: "HushTests-\(UUID().uuidString)")
        try await store.delete()
        #expect(!(await store.hasCredential()))
    }

    private func storedAttributes(service: String, account: String) -> [String: Any]? {
        let context = LAContext()
        context.interactionNotAllowed = true
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecUseDataProtectionKeychain as String: true,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess else { return nil }
        return item as? [String: Any]
    }
}
