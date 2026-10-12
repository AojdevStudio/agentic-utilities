import Foundation

// Pure vault values. Nothing in this file talks to the bridge, the Keychain, or UIKit.

/// Encodes as a bare UUID string, like `ProjectID`.
nonisolated struct SecretID: Hashable, Codable, Sendable {
    let rawValue: UUID

    init(_ rawValue: UUID) { self.rawValue = rawValue }

    init(from decoder: any Decoder) throws {
        rawValue = try decoder.singleValueContainer().decode(UUID.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

nonisolated struct ProjectID: Hashable, Codable, Sendable {
    let rawValue: UUID

    init(_ rawValue: UUID) { self.rawValue = rawValue }

    init(from decoder: any Decoder) throws {
        rawValue = try decoder.singleValueContainer().decode(UUID.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

/// Not a secret.
nonisolated enum Region: String, Codable, CaseIterable, Sendable {
    case us, eu

    var bridgeCode: UInt32 {
        switch self {
        case .us: 0
        case .eu: 1
        }
    }
}

/// The server vault behind a session. Token rotation does not change this identity.
nonisolated struct VaultIdentity: Hashable, Sendable {
    let region: Region
    let organizationID: UUID
}

nonisolated struct Project: Identifiable, Hashable, Sendable {
    let id: ProjectID
    let name: String
}

/// Secret metadata. Never holds a value or a note.
nonisolated struct SecretSummary: Identifiable, Hashable, Sendable {
    let id: SecretID
    let key: String
    let projectIDs: [ProjectID]
}

/// Metadata from one refresh. Rows are sorted by name, then by ID, so equal names keep a stable order.
nonisolated struct Catalog: Hashable, Sendable {
    let projects: [Project]
    let secrets: [SecretSummary]
    private let projectsByID: [ProjectID: Project]

    init(projects: [Project], secrets: [SecretSummary]) {
        self.projects = projects.sorted { Self.precedes($0.name, $0.id.rawValue, $1.name, $1.id.rawValue) }
        self.secrets = secrets.sorted { Self.precedes($0.key, $0.id.rawValue, $1.key, $1.id.rawValue) }
        projectsByID = Dictionary(projects.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    }

    func project(_ id: ProjectID) -> Project? { projectsByID[id] }

    /// Rows whose key or project name contains `query`, ignoring case and diacritics,
    /// limited to `project` when one is given.
    func matching(query: String, project: ProjectID?) -> [SecretSummary] {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return secrets.filter { secret in
            if let project, !secret.projectIDs.contains(project) { return false }
            if needle.isEmpty { return true }
            if Self.contains(secret.key, needle) { return true }
            return secret.projectIDs.contains { id in
                projectsByID[id].map { Self.contains($0.name, needle) } ?? false
            }
        }
    }

    func adding(_ secret: SecretSummary) -> Catalog {
        Catalog(projects: projects, secrets: secrets.filter { $0.id != secret.id } + [secret])
    }

    private static func contains(_ haystack: String, _ needle: String) -> Bool {
        haystack.range(of: needle, options: [.caseInsensitive, .diacriticInsensitive]) != nil
    }

    private static func precedes(_ lhs: String, _ lhsID: UUID, _ rhs: String, _ rhsID: UUID) -> Bool {
        switch lhs.localizedStandardCompare(rhs) {
        case .orderedAscending: true
        case .orderedDescending: false
        case .orderedSame: lhsID.uuidString < rhsID.uuidString
        }
    }
}

/// One secret with its value. Printing or reflecting it never shows the value.
nonisolated struct SecretDetail: Identifiable, Hashable, Sendable {
    let id: SecretID
    let key: String
    let value: String
    let note: String
    let projectID: ProjectID?

    var summary: SecretSummary {
        SecretSummary(id: id, key: key, projectIDs: projectID.map { [$0] } ?? [])
    }
}

extension SecretDetail: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String { "SecretDetail(id: \(id.rawValue), key: \(key), value: <redacted>)" }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: ["id": id, "key": key], displayStyle: .struct) }
}

/// A validated draft for `VaultStore.create`. Printing it never shows the value or note.
nonisolated struct NewSecret: Sendable {
    let key: String
    let value: String
    let note: String?
    let projectID: ProjectID

    enum Invalid: Error, Hashable, Sendable {
        case emptyKey, emptyValue, missingProject, unknownProject
    }

    /// Trims the key. Keeps value and note as typed. The project must exist in `catalog`.
    init(key: String, value: String, note: String?, projectID: ProjectID?, in catalog: Catalog) throws(Invalid) {
        let trimmedKey = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw .emptyKey }
        guard !value.isEmpty else { throw .emptyValue }
        guard let projectID else { throw .missingProject }
        guard catalog.project(projectID) != nil else { throw .unknownProject }
        self.key = trimmedKey
        self.value = value
        self.note = note
        self.projectID = projectID
    }
}

extension NewSecret: CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable {
    var description: String { "NewSecret(key: \(key), value: <redacted>)" }
    var debugDescription: String { description }
    var customMirror: Mirror { Mirror(self, children: ["key": key, "projectID": projectID], displayStyle: .struct) }
}

/// A create whose result is unknown. Metadata only, held in memory only.
nonisolated struct CreateAttempt: Identifiable, Hashable, Sendable {
    let id: UUID
    let key: String
    let projectID: ProjectID
    let priorIDs: Set<SecretID>
    let startedAt: Date
    let vaultIdentity: VaultIdentity

    /// Secrets that were not there before the attempt and share its key and project.
    /// Evidence for the user to review, not proof that this attempt wrote them.
    func candidates(in catalog: Catalog) -> [SecretSummary] {
        catalog.secrets.filter {
            !priorIDs.contains($0.id) && $0.key == key && $0.projectIDs.contains(projectID)
        }
    }
}

nonisolated enum CreateOutcome: Hashable, Sendable {
    case created(SecretDetail)
    case rejected(VaultFailure)
    /// The server may or may not have the secret. Never retried automatically.
    case unconfirmed(CreateAttempt)
}

/// Every failure the views can see. Raw bridge, SDK, and Keychain errors never pass through.
nonisolated enum VaultFailure: Error, Hashable, Sendable {
    case invalidInput
    case invalidToken
    case authentication
    case permission
    case network
    case locked
    case notConfigured
    case deviceAuthenticationCancelled
    case deviceAuthenticationUnavailable
    case deviceAuthenticationFailed
    case createPending
    case generic
}

nonisolated enum SessionPhase: Hashable, Sendable {
    case disconnected
    case locked
    case unlocking
    case unlocked(Catalog)
}
