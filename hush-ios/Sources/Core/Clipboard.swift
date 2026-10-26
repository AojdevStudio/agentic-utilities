import UIKit
import UniformTypeIdentifiers

/// Copies one value for this device only, with a system expiry and a matching cleanup timer.
@MainActor final class Clipboard {
    private let pasteboard: UIPasteboard
    private let lifetime: Duration
    private var ownedChangeCount: Int?
    /// Our value while another app may paste it. `reclaim` makes it ours again.
    private var releasedChangeCount: Int?
    private var expiry: Task<Void, Never>?

    init(pasteboard: UIPasteboard = .general, lifetime: Duration = .seconds(30)) {
        self.pasteboard = pasteboard
        self.lifetime = lifetime
    }

    func copy(_ value: String) {
        pasteboard.setItems(
            [[UTType.utf8PlainText.identifier: value]],
            options: Self.options(now: .now, lifetime: lifetime)
        )
        ownedChangeCount = pasteboard.changeCount
        releasedChangeCount = nil
        expiry?.cancel()
        expiry = Task { [weak self, lifetime] in
            do {
                try await Task.sleep(for: lifetime)
            } catch {
                // Sleep throws only on cancellation: a newer copy or clearIfOwned owns cleanup now.
                return
            }
            self?.clearIfOwned()
        }
    }

    /// Clears the pasteboard only if nothing was copied after our value.
    func clearIfOwned() {
        expiry?.cancel()
        expiry = nil
        guard let owned = ownedChangeCount else { return }
        ownedChangeCount = nil
        guard pasteboard.changeCount == owned else { return }
        pasteboard.items = []
    }

    /// Lets another app paste our value. `clearIfOwned` skips it until `reclaim`.
    /// The system expiry set in `copy` still removes it.
    func release() {
        expiry?.cancel()
        expiry = nil
        if let owned = ownedChangeCount { releasedChangeCount = owned }
        ownedChangeCount = nil
    }

    /// Makes a released value ours again if nothing was copied after it, so a lock clears it.
    func reclaim() {
        guard let released = releasedChangeCount else { return }
        releasedChangeCount = nil
        guard pasteboard.changeCount == released else { return }
        ownedChangeCount = released
    }

    static func options(now: Date, lifetime: Duration) -> [UIPasteboard.OptionsKey: Any] {
        let seconds = Double(lifetime.components.seconds) + Double(lifetime.components.attoseconds) / 1e18
        return [.localOnly: true, .expirationDate: now.addingTimeInterval(seconds)]
    }
}
