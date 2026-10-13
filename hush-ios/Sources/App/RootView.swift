import SwiftUI

/// Picks the screen for the current session phase.
struct RootView: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            switch model.route {
            case .launching:
                Color.hushBackground.ignoresSafeArea()
            case .connect:
                ConnectView(model: model)
            case .locked:
                LockedView(model: model)
            case .vault(let catalog):
                VaultView(model: model, catalog: catalog)
            }
        }
        .onChange(of: model.store.phase.catalog == nil) { _, isClosed in
            // The store can lock on its own, for example on a poisoned session. Drop what it showed.
            if isClosed { model.clearVault() }
        }
    }
}

/// Tells the user an earlier add has no confirmed result. Shown outside the vault, so a lock or
/// forget never hides it. Review needs an unlocked refresh.
struct PendingCreateNotice: View {
    let attempt: CreateAttempt

    var body: some View {
        Label {
            Text("An add of \(Text(attempt.key).monospaced()) is unconfirmed. After you unlock, refresh and review it.")
        } icon: {
            Image(systemName: "questionmark.circle")
        }
        .font(.callout)
        .accessibilityIdentifier("pending-notice")
    }
}
