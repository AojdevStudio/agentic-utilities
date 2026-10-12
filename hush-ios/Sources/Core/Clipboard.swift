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
        expiry?.cancel()
        expiry = nil
        guard let owned = ownedChangeCount else { return }
        ownedChangeCount = nil
        guard pasteboard.changeCount == owned else { return }
        pasteboard.items = []
    }

    static func options(now: Date, lifetime: Duration) -> [UIPasteboard.OptionsKey: Any] {
        let seconds = Double(lifetime.components.seconds) + Double(lifetime.components.attoseconds) / 1e18
        return [.localOnly: true, .expirationDate: now.addingTimeInterval(seconds)]
    }
}
