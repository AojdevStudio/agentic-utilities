import Foundation

nonisolated enum BridgeRequest: Sendable {
    case login(token: String)
    case catalog
    case read(SecretID)
    case create(NewSecret)
}

extension BridgeRequest: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String {
        switch self {
        case .login: "login(<redacted>)"
        case .catalog: "catalog"
        case .read(let id): "read(\(id.rawValue))"
        case .create(let draft): "create(\(draft))"
        }
    }

    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: ["request": description]) }
}

nonisolated enum BridgeError: Error, Hashable, Sendable {
    case rejected(code: String)
    case malformed

    var failure: VaultFailure {
        guard case .rejected(let code) = self else { return .generic }
        return switch code {
        case "invalid_request": .invalidInput
        case "invalid_token": .invalidToken
        case "authentication": .authentication
        case "permission": .permission
        case "network": .network
        case "locked": .locked
        default: .generic
        }
    }

    /// True when the session may be poisoned and must be dropped.
    var poisonsSession: Bool {
        switch self {
        case .malformed: true
        case .rejected(let code): code == "locked" || code == "internal"
        }
    }

    /// True only for the codes Bridge/contract.md lists as definite for create: nothing was stored.
    var provesNothingWritten: Bool {
        guard case .rejected(let code) = self else { return false }
        return ["invalid_request", "authentication", "permission", "locked", "network"].contains(code)
    }
}

nonisolated enum BridgeWire {
    struct LoginData: Decodable, Sendable {
        let organizationID: UUID
    }

    struct CatalogData: Decodable, Sendable {
        struct ProjectRow: Decodable, Sendable { let id: ProjectID; let name: String }
        struct SecretRow: Decodable, Sendable { let id: SecretID; let key: String; let projectIDs: [ProjectID] }

        let projects: [ProjectRow]
        let secrets: [SecretRow]

        var catalog: Catalog {
            Catalog(
                projects: projects.map { Project(id: $0.id, name: $0.name) },
                secrets: secrets.map { SecretSummary(id: $0.id, key: $0.key, projectIDs: $0.projectIDs) }
            )
        }
    }

    struct DetailData: Decodable, Sendable {
        let id: SecretID
        let key: String
        let value: String
        let note: String?
        let projectID: ProjectID?

        var detail: SecretDetail {
            SecretDetail(id: id, key: key, value: value, note: note ?? "", projectID: projectID)
        }
    }

    private struct Envelope<Payload: Decodable>: Decodable {
        struct Failure: Decodable { let code: String }
        let ok: Bool
        let data: Payload?
        let error: Failure?
    }

    private struct Wire: Encodable {
        let op: String
        var token: String?
        var id: SecretID?
        var key: String?
        var value: String?
        var note: String?
        var projectID: ProjectID?
    }

    static func encode(_ request: BridgeRequest) throws(BridgeError) -> Data {
        let wire = switch request {
        case .login(let token): Wire(op: "login", token: token)
        case .catalog: Wire(op: "catalog")
        case .read(let id): Wire(op: "read", id: id)
        case .create(let draft):
            Wire(op: "create", key: draft.key, value: draft.value, note: draft.note ?? "", projectID: draft.projectID)
        }
        do {
            return try JSONEncoder().encode(wire)
        } catch {
            throw .malformed
        }
    }

    static func decode<Payload: Decodable>(_: Payload.Type, from reply: Data) throws(BridgeError) -> Payload {
        let envelope: Envelope<Payload>
        do {
            envelope = try JSONDecoder().decode(Envelope<Payload>.self, from: reply)
        } catch {
            throw .malformed
        }
        switch (envelope.ok, envelope.data, envelope.error) {
        case (true, let data?, nil): return data
        case (false, nil, let failure?): throw .rejected(code: failure.code)
        default: throw .malformed
        }
    }
}
