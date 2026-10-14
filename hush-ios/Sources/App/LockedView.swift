import SwiftUI

/// A token is saved. Unlock shows the device authentication prompt. Never unlocks on its own,
/// so a cancelled prompt cannot loop.
struct LockedView: View {
    @Bindable var model: AppModel
    @State private var confirmingForget = false

    private var isUnlocking: Bool { model.store.phase == .unlocking }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        HushWordmark()
                        Text("Locked").foregroundStyle(.secondary)
                    }
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }

                Section {
                    Button {
                        Task { await model.unlock() }
                    } label: {
                        Label(isUnlocking ? "Unlocking" : "Unlock", systemImage: "faceid")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(isUnlocking)
                    .keyboardShortcut(.defaultAction)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                    .accessibilityIdentifier("unlock-button")
                }

                if let failure = model.unlockFailure {
                    Section { FailureMessage(failure: failure, context: .unlock) }
                }
                if let failure = model.forgetFailure {
                    Section { FailureMessage(failure: failure, context: .unlock) }
                }

                if let attempt = model.store.pendingCreate {
                    Section { PendingCreateNotice(attempt: attempt) }
                }

                Section {
                    Button("Replace access token") { model.beginReplacingToken() }
                        .disabled(isUnlocking)
                        .accessibilityIdentifier("replace-token-button")
                    Button("Forget saved token", role: .destructive) { confirmingForget = true }
                        .disabled(isUnlocking)
                        .accessibilityIdentifier("forget-button")
                }
            }
            .hushListBackground()
            .confirmationDialog("Forget the saved token?", isPresented: $confirmingForget, titleVisibility: .visible) {
                Button("Forget", role: .destructive) { Task { await model.forget() } }
            } message: {
                Text("Hush deletes the token from this device. Secrets in Bitwarden do not change.")
            }
        }
    }
}
