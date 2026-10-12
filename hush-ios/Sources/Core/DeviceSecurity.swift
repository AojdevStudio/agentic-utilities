import Foundation
import LocalAuthentication
import Security

/// The saved machine access token and the region it signs in to. Stored as one Keychain record.
nonisolated struct Credential: Codable, Hashable, Sendable {
    let token: String
    let region: Region
}

extension Credential: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String { "Credential(region: \(region.rawValue), token: <redacted>)" }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: ["region": region], displayStyle: .struct) }
}

nonisolated protocol CredentialStore: Sendable {
    /// True when a credential is saved. Never prompts.
    func hasCredential() async -> Bool
    /// Reads the credential. Shows the device authentication prompt.
    func load() async throws(VaultFailure) -> Credential
    /// Replaces any saved credential.
    func save(_ credential: Credential) async throws(VaultFailure)
    /// Deletes the credential without a prompt. Succeeds when nothing is saved.
    func delete() async throws(VaultFailure)
}

nonisolated protocol DeviceAuthenticator: Sendable {
    func authenticate() async throws(VaultFailure)
}

/// A device-only Keychain generic password behind user presence. Never synchronized.
/// Every Keychain call runs on a private serial queue because a prompting read blocks its thread.
nonisolated final class KeychainCredentialStore: CredentialStore {
    let service: String
    let account: String
    private let prompt: String
    // A narrow seam for failures from the external Keychain write APIs.
    private let addItem: @Sendable (CFDictionary) -> OSStatus
    private let updateItem: @Sendable (CFDictionary, CFDictionary) -> OSStatus
    private let queue = DispatchQueue(label: "Hush.KeychainCredentialStore")

    init(
        service: String = "Hush",
        account: String = "machine-credential",
        prompt: String = "Unlock Hush",
        addItem: @escaping @Sendable (CFDictionary) -> OSStatus = { SecItemAdd($0, nil) },
        updateItem: @escaping @Sendable (CFDictionary, CFDictionary) -> OSStatus = { SecItemUpdate($0, $1) }
    ) {
        self.service = service
        self.account = account
        self.prompt = prompt
        self.addItem = addItem
        self.updateItem = updateItem
    }

    func hasCredential() async -> Bool {
        let result: Result<Bool, VaultFailure> = await run { [self] in
            let context = LAContext()
            context.interactionNotAllowed = true
            var query = baseQuery
            query[kSecReturnAttributes as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            query[kSecUseAuthenticationContext as String] = context
            let status = SecItemCopyMatching(query as CFDictionary, nil)
            return .success(status == errSecSuccess || status == errSecInteractionNotAllowed)
        }
        return (try? result.get()) ?? false
    }

    func load() async throws(VaultFailure) -> Credential {
        try await run { [self] () -> Result<Credential, VaultFailure> in
            let context = LAContext()
            context.localizedReason = prompt
            var query = baseQuery
            query[kSecReturnData as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
            query[kSecUseAuthenticationContext as String] = context
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            guard status == errSecSuccess else { return .failure(Self.failure(for: status)) }
            guard let data = item as? Data, let credential = try? JSONDecoder().decode(Credential.self, from: data) else {
                return .failure(.generic)
            }
            return .success(credential)
        }.get()
    }

    func save(_ credential: Credential) async throws(VaultFailure) {
        try await run { [self] () -> Result<Void, VaultFailure> in
            guard let data = try? JSONEncoder().encode(credential) else { return .failure(.generic) }
            let context = LAContext()
            context.localizedReason = prompt
            var query = baseQuery
            query[kSecUseAuthenticationContext as String] = context
            // Updating only the value keeps the existing access control and device-only policy.
            let updated = updateItem(query as CFDictionary, [kSecValueData as String: data] as CFDictionary)
            if updated == errSecSuccess { return .success(()) }
            guard updated == errSecItemNotFound else {
                return .failure(Self.failure(for: updated))
            }

            var error: Unmanaged<CFError>?
            guard let access = SecAccessControlCreateWithFlags(
                nil, kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly, .userPresence, &error
            ) else {
                error?.release()
                return .failure(.generic)
            }
            var attributes = baseQuery
            attributes[kSecValueData as String] = data
            attributes[kSecAttrAccessControl as String] = access
            attributes[kSecAttrSynchronizable as String] = kCFBooleanFalse
            let status = addItem(attributes as CFDictionary)
            return status == errSecSuccess ? .success(()) : .failure(Self.failure(for: status))
        }.get()
    }

    func delete() async throws(VaultFailure) {
        try await run { [self] () -> Result<Void, VaultFailure> in
            let status = SecItemDelete(baseQuery as CFDictionary)
            return status == errSecSuccess || status == errSecItemNotFound ? .success(()) : .failure(Self.failure(for: status))
        }.get()
    }

    private var baseQuery: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecUseDataProtectionKeychain as String: true,
        ]
    }

    private func run<Success: Sendable>(
        _ work: @escaping @Sendable () -> Result<Success, VaultFailure>
    ) async -> Result<Success, VaultFailure> {
        await withCheckedContinuation { continuation in
            queue.async { continuation.resume(returning: work()) }
        }
    }

    private static func failure(for status: OSStatus) -> VaultFailure {
        switch status {
        case errSecItemNotFound: .notConfigured
        case errSecUserCanceled: .deviceAuthenticationCancelled
        case errSecAuthFailed: .deviceAuthenticationFailed
        case errSecNotAvailable, errSecInteractionNotAllowed: .deviceAuthenticationUnavailable
        default: .generic
        }
    }
}

/// Device owner authentication: biometrics with passcode fallback.
nonisolated struct LocalDeviceAuthenticator: DeviceAuthenticator {
    var reason = "Save this access token on this device"

    func authenticate() async throws(VaultFailure) {
        let context = LAContext()
        do {
            _ = try await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)
        } catch let error as LAError {
            switch error.code {
            case .userCancel, .appCancel, .systemCancel, .userFallback: throw .deviceAuthenticationCancelled
            case .passcodeNotSet: throw .deviceAuthenticationUnavailable
            default: throw .deviceAuthenticationFailed
            }
        } catch {
            throw .deviceAuthenticationFailed
        }
    }
}
