import Foundation
import Observation

/// The app's one vault session. Views read `phase` and call these methods. API.md has the contract.
///
/// Every session change bumps `generation` first. Every await is followed by a generation check,
/// so a call that started before `lock()` or `forget()` can never unlock or repopulate the store.
/// A stale create returns only its metadata attempt.
@MainActor @Observable final class VaultStore {
    private(set) var phase: SessionPhase = .disconnected
    private(set) var pendingCreate: CreateAttempt?
    private(set) var canAcknowledgePendingCreate = false
    private(set) var vaultIdentity: VaultIdentity?
    private(set) var pendingCreateSettled = false

    @ObservationIgnored private let credentials: any CredentialStore
    @ObservationIgnored private let deviceAuthenticator: any DeviceAuthenticator
    @ObservationIgnored private let makeConnection: ConnectionFactory
    @ObservationIgnored private var connection: (any SecretConnection)?
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var hasSavedCredential = false

    init(credentials: any CredentialStore, deviceAuthenticator: any DeviceAuthenticator, makeConnection: @escaping ConnectionFactory) {
        self.credentials = credentials
        self.deviceAuthenticator = deviceAuthenticator
        self.makeConnection = makeConnection
    }

    func restore() async {
        guard phase == .disconnected else { return }
        let started = generation
        let saved = await credentials.hasCredential()
        guard started == generation, phase == .disconnected else { return }
        hasSavedCredential = saved
        if saved { phase = .locked }
    }

    func connect(token: String, region: Region) async throws(VaultFailure) {
        let started = beginSession()
        let connection = makeConnection(region)
        self.connection = connection
        do throws(VaultFailure) {
            let catalog = try await signIn(token: token, region: region, on: connection, started: started)
            try await deviceAuthenticator.authenticate()
            try ensureCurrent(started)
            try await credentials.save(Credential(token: token, region: region))
            hasSavedCredential = true
            if started != generation, phase == .disconnected { phase = .locked }
            try ensureCurrent(started)
            phase = .unlocked(catalog)
        } catch {
            endSession(started)
            throw error
        }
    }

    func unlock() async throws(VaultFailure) {
        switch phase {
        case .unlocked: return
        case .unlocking: throw .locked
        case .disconnected: throw .notConfigured
        case .locked: break
        }
        let started = beginSession()
        let credential: Credential
        do throws(VaultFailure) {
            credential = try await credentials.load()
            try ensureCurrent(started)
        } catch {
            if error == .notConfigured { hasSavedCredential = false }
            endSession(started)
            throw error
        }
        let connection = makeConnection(credential.region)
        self.connection = connection
        do throws(VaultFailure) {
            phase = .unlocked(try await signIn(token: credential.token, region: credential.region, on: connection, started: started))
        } catch {
            endSession(started)
            throw error
        }
    }

    func refresh() async throws(VaultFailure) {
        let (connection, started) = try activeSession()
        let reviewable = pendingCreateSettled && pendingCreate?.vaultIdentity == vaultIdentity
            ? pendingCreate?.id : nil
        let data: BridgeWire.CatalogData = try await call(.catalog, on: connection, started: started)
        phase = .unlocked(data.catalog)
        if let reviewable, pendingCreate?.id == reviewable { canAcknowledgePendingCreate = true }
    }

    func read(_ id: SecretID) async throws(VaultFailure) -> SecretDetail {
        let (connection, started) = try activeSession()
        let data: BridgeWire.DetailData = try await call(.read(id), on: connection, started: started)
        guard data.id == id else { throw failure(for: BridgeError.malformed) }
        return data.detail
    }

    func create(_ draft: NewSecret) async -> CreateOutcome {
        guard pendingCreate == nil else { return .rejected(.createPending) }
        guard case .unlocked(let catalog) = phase, let connection, let vaultIdentity else {
            return .rejected(.locked)
        }
        guard catalog.project(draft.projectID) != nil else { return .rejected(.invalidInput) }

        let started = generation
        let attempt = CreateAttempt(
            id: UUID(), key: draft.key, projectID: draft.projectID,
            priorIDs: Set(catalog.secrets.map(\.id)), startedAt: .now, vaultIdentity: vaultIdentity
        )
        pendingCreate = attempt
        pendingCreateSettled = false
        canAcknowledgePendingCreate = false
        defer { if pendingCreate?.id == attempt.id { pendingCreateSettled = true } }

        do {
            let reply = try await connection.send(.create(draft))
            guard started == generation else { return .unconfirmed(attempt) }
            let detail = try BridgeWire.decode(BridgeWire.DetailData.self, from: reply).detail
            pendingCreate = nil
            if case .unlocked(let current) = phase { phase = .unlocked(current.adding(detail.summary)) }
            return .created(detail)
        } catch {
            guard started == generation else { return .unconfirmed(attempt) }
            let failure = failure(for: error)
            if Self.provesNothingWritten(error) {
                pendingCreate = nil
                return .rejected(failure)
            }
            return .unconfirmed(attempt)
        }
    }

    func acknowledgePendingCreate() throws(VaultFailure) {
        guard let attempt = pendingCreate else { return }
        guard case .unlocked = phase, vaultIdentity == attempt.vaultIdentity,
              canAcknowledgePendingCreate else { throw .createPending }
        pendingCreate = nil
        canAcknowledgePendingCreate = false
    }

    func lock() {
        invalidate()
        phase = restingPhase
    }

    func forget() async throws(VaultFailure) {
        lock()
        let started = generation
        try await credentials.delete()
        hasSavedCredential = false
        if started == generation { phase = .disconnected }
    }

    private var restingPhase: SessionPhase { hasSavedCredential ? .locked : .disconnected }

    private func invalidate() {
        generation &+= 1
        connection?.close()
        connection = nil
        vaultIdentity = nil
        canAcknowledgePendingCreate = false
    }

    private func beginSession() -> UInt64 {
        invalidate()
        phase = .unlocking
        return generation
    }

    private func endSession(_ started: UInt64) {
        guard started == generation else { return }
        lock()
    }

    private func ensureCurrent(_ started: UInt64) throws(VaultFailure) {
        guard started == generation else { throw .locked }
    }

    private func activeSession() throws(VaultFailure) -> (any SecretConnection, UInt64) {
        guard case .unlocked = phase, let connection else { throw .locked }
        return (connection, generation)
    }

    private func signIn(
        token: String, region: Region, on connection: any SecretConnection, started: UInt64
    ) async throws(VaultFailure) -> Catalog {
        let login: BridgeWire.LoginData = try await call(.login(token: token), on: connection, started: started)
        let data: BridgeWire.CatalogData = try await call(.catalog, on: connection, started: started)
        vaultIdentity = VaultIdentity(region: region, organizationID: login.organizationID)
        return data.catalog
    }

    private func call<Payload: Decodable>(
        _ request: BridgeRequest, on connection: any SecretConnection, started: UInt64
    ) async throws(VaultFailure) -> Payload {
        let reply: Data
        do {
            reply = try await connection.send(request)
        } catch {
            try ensureCurrent(started)
            throw failure(for: error)
        }
        try ensureCurrent(started)
        do {
            return try BridgeWire.decode(Payload.self, from: reply)
        } catch {
            throw failure(for: error)
        }
    }

    /// Maps a send or decode error to a view-safe failure, and locks when the session may be poisoned.
    private func failure(for error: any Error) -> VaultFailure {
        switch error {
        case let bridge as BridgeError:
            if bridge.poisonsSession { lock() }
            return bridge.failure
        case let failure as VaultFailure:
            if failure == .locked { lock() }
            return failure
        default:
            return .generic
        }
    }

    private static func provesNothingWritten(_ error: any Error) -> Bool {
        switch error {
        case let bridge as BridgeError: bridge.provesNothingWritten
        case let failure as VaultFailure: failure == .locked
        default: false
        }
    }
}
