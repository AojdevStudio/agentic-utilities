import UIKit
import UniformTypeIdentifiers

/// Copies one value for this device only, with a system expiry and a matching cleanup timer.
@MainActor final class Clipboard {
    private let pasteboard: UIPasteboard
    private let lifetime: Duration
    private var ownedChangeCount: Int?
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
        let owned = ownedChangeCount
        release()
        guard let owned, pasteboard.changeCount == owned else { return }
        pasteboard.items = []
    }

    /// Stops tracking our value without clearing it, so another app can paste it.
    /// The system expiry set in `copy` still removes it.
    func release() {
        expiry?.cancel()
        expiry = nil
        ownedChangeCount = nil
    }

    static func options(now: Date, lifetime: Duration) -> [UIPasteboard.OptionsKey: Any] {
        let seconds = Double(lifetime.components.seconds) + Double(lifetime.components.attoseconds) / 1e18
        return [.localOnly: true, .expirationDate: now.addingTimeInterval(seconds)]
    }
}
