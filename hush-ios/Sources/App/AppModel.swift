import Observation
import SwiftUI

/// What the root view shows.
enum Route: Equatable {
    case launching
    case connect
    case locked
    case vault(Catalog)
}

/// One fetched secret, owned here and never by the store. Cleared on lock and background.
enum DetailState: Equatable {
    case idle
    case loading(SecretID)
    case loaded(SecretDetail)
    case failed(SecretID, VaultFailure)

    var id: SecretID? {
        switch self {
        case .idle: nil
        case .loading(let id), .failed(let id, _): id
        case .loaded(let detail): detail.id
        }
    }
}

/// The add form. Value and note live only in memory and are cleared on lock and background.
struct Draft: Equatable {
    var key = ""
    var value = ""
    var note = ""
    var projectID: ProjectID?
    var invalid: NewSecret.Invalid?
    var failure: VaultFailure?
}

enum Sheet: String, Identifiable {
    case create, settings
    var id: String { rawValue }
}

extension SessionPhase {
    var catalog: Catalog? {
        if case .unlocked(let catalog) = self { catalog } else { nil }
    }
}

/// App state for the views: route, selection, the open detail, the add draft, and sheets.
/// Vault work goes only through `store`. Every async result is checked against a generation
/// so a reply that started before a lock, or before a newer request, never repopulates the screen.
@MainActor @Observable final class AppModel {
    let store: VaultStore

    private(set) var hasRestored = false
    var isReplacingToken = false
    private(set) var isConnecting = false
    var tokenDraft = ""
    private(set) var connectFailure: VaultFailure?
    private(set) var unlockFailure: VaultFailure?
    private(set) var forgetFailure: VaultFailure?

    var query = ""
    var projectFilter: ProjectID?
    var selection: SecretID? {
        didSet {
            guard selection != oldValue else { return }
            isRevealed = false
            copiedSecret = nil
            if detail.id != selection {
                detailGeneration &+= 1
                detail = .idle
            }
        }
    }
    private(set) var detail: DetailState = .idle
    var isRevealed = false
    private(set) var copiedSecret: SecretID?

    var sheet: Sheet?
    var draft = Draft()
    private(set) var isSaving = false
    private(set) var isRefreshing = false
    private(set) var vaultFailure: VaultFailure?

    @ObservationIgnored private let clipboard = Clipboard()
    /// Bumped whenever the vault view is cleared. Every async call compares against it.
    @ObservationIgnored private var epoch: UInt64 = 0
    /// Bumped for each detail request, so read A finishing after read B is dropped.
    @ObservationIgnored private var detailGeneration: UInt64 = 0

    init(store: VaultStore) {
        self.store = store
    }

    var route: Route {
        guard hasRestored else { return .launching }
        switch store.phase {
        case .disconnected: return .connect
        case .locked: return isReplacingToken ? .connect : .locked
        case .unlocking: return isConnecting ? .connect : .locked
        case .unlocked(let catalog): return .vault(catalog)
        }
    }

    // MARK: Session

    func restore() async {
        await store.restore()
        hasRestored = true
    }

    /// Connects with the typed token. Returns true on success so the view can remember the region.
    func connect(region: Region) async -> Bool {
        let token = tokenDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !token.isEmpty, !isConnecting else { return false }
        let started = epoch
        isConnecting = true
        connectFailure = nil
        defer { if started == epoch { isConnecting = false } }
        do throws(VaultFailure) {
            try await store.connect(token: token, region: region)
            guard started == epoch else { return false }
            tokenDraft = ""
            isReplacingToken = false
            unlockFailure = nil
            return true
        } catch {
            if started == epoch { connectFailure = error }
            return false
        }
    }

    func unlock() async {
        let started = epoch
        unlockFailure = nil
        forgetFailure = nil
        do throws(VaultFailure) {
            try await store.unlock()
        } catch {
            if started == epoch { unlockFailure = error }
        }
    }

    func beginReplacingToken() {
        tokenDraft = ""
        connectFailure = nil
        isReplacingToken = true
    }

    func cancelReplacingToken() {
        tokenDraft = ""
        connectFailure = nil
        isReplacingToken = false
    }

    func forget() async {
        forgetFailure = nil
        lock()
        do throws(VaultFailure) {
            try await store.forget()
            isReplacingToken = false
            unlockFailure = nil
        } catch {
            forgetFailure = error
        }
    }

    /// Locks now. Drops the detail, the draft, sheets, and our clipboard value.
    func lock() {
        store.lock()
        clearVault()
    }

    /// Scene went to the background. Release the copied value to its system expiry, so another app
    /// can paste it. Then lock and clear every secret on screen.
    func enterBackground() {
        clipboard.release()
        lock()
        tokenDraft = ""
        isConnecting = false
        isReplacingToken = false
        connectFailure = nil
    }

    /// Drops everything that came from an unlocked session. Also called when the store locks itself.
    func clearVault() {
        clipboard.clearIfOwned()
        epoch &+= 1
        detailGeneration &+= 1
        sheet = nil
        draft = Draft()
        isSaving = false
        isRefreshing = false
        vaultFailure = nil
        selection = nil
        detail = .idle
        isRevealed = false
        copiedSecret = nil
    }

    // MARK: Vault

    func refresh() async {
        guard !isRefreshing else { return }
        let started = epoch
        isRefreshing = true
        defer { if started == epoch { isRefreshing = false } }
        do throws(VaultFailure) {
            try await store.refresh()
            guard started == epoch else { return }
            vaultFailure = nil
            if let filter = projectFilter, let catalog = store.phase.catalog, catalog.project(filter) == nil {
                projectFilter = nil
            }
        } catch {
            if started == epoch { vaultFailure = error }
        }
    }

    /// Fetches the selected secret on demand. Skips the fetch when it is already on screen.
    func loadDetail(_ id: SecretID) async {
        if case .loaded(let current) = detail, current.id == id { return }
        detailGeneration &+= 1
        let request = detailGeneration
        detail = .loading(id)
        isRevealed = false
        do throws(VaultFailure) {
            let fetched = try await store.read(id)
            guard request == detailGeneration, selection == id, !Task.isCancelled else { return }
            detail = .loaded(fetched)
        } catch {
            guard request == detailGeneration, selection == id, !Task.isCancelled else { return }
            detail = .failed(id, error)
        }
    }

    func retryDetail() async {
        guard let id = selection else { return }
        detailGeneration &+= 1
        detail = .idle
        await loadDetail(id)
    }

    func copyValue() {
        guard case .loaded(let current) = detail, current.id == selection else { return }
        clipboard.copy(current.value)
        copiedSecret = current.id
    }

    // MARK: Add

    func openCreate() {
        guard case .unlocked(let catalog) = store.phase else { return }
        if draft.projectID == nil {
            draft.projectID = projectFilter.flatMap { catalog.project($0)?.id }
                ?? (catalog.projects.count == 1 ? catalog.projects.first?.id : nil)
        }
        draft.failure = nil
        sheet = .create
    }

    func cancelCreate() {
        draft = Draft()
        sheet = nil
    }

    /// Validates, then sends the create once. Never retries.
    func saveDraft() async {
        guard !isSaving, case .unlocked(let catalog) = store.phase else { return }
        let note = draft.note.isEmpty ? nil : draft.note
        let secret: NewSecret
        do throws(NewSecret.Invalid) {
            secret = try NewSecret(key: draft.key, value: draft.value, note: note, projectID: draft.projectID, in: catalog)
        } catch {
            draft.invalid = error
            return
        }
        draft.invalid = nil
        draft.failure = nil
        let started = epoch
        isSaving = true
        let outcome = await store.create(secret)
        guard started == epoch else { return }
        isSaving = false
        switch outcome {
        case .created(let created):
            draft = Draft()
            sheet = nil
            showCreated(created)
        case .rejected(let failure):
            draft.failure = failure
        case .unconfirmed:
            // The server may have it. Keep key and project for review, drop the value and note.
            draft.value = ""
            draft.note = ""
            sheet = nil
        }
    }

    func acknowledgePendingCreate() {
        do throws(VaultFailure) {
            try store.acknowledgePendingCreate()
            draft = Draft()
        } catch {
            vaultFailure = error
        }
    }

    /// Opens the secret the create returned, without a second read.
    private func showCreated(_ created: SecretDetail) {
        query = ""
        if let filter = projectFilter, created.projectID != filter { projectFilter = nil }
        selection = created.id
        detailGeneration &+= 1
        detail = .loaded(created)
        isRevealed = false
    }
}
