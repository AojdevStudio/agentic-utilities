import SwiftUI

/// Local settings: region shown, appearance, lock, and forget the saved token.
struct SettingsView: View {
    let model: AppModel
    @AppStorage(AppearanceKey.darkMode) private var darkMode = false
    @AppStorage(AppearanceKey.region) private var region: Region = .us
    @Environment(\.dismiss) private var dismiss
    @State private var confirmingForget = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Region", value: region.name)
                        .accessibilityIdentifier("settings-region")
                    Toggle("Dark mode", isOn: $darkMode)
                        .accessibilityIdentifier("settings-dark-mode")
                } footer: {
                    Text("Copied values stay on this device and clear after 30 seconds.")
                }

                Section {
                    NavigationLink { NoticesView() } label: {
                        Text("Third-party notices")
                            .foregroundStyle(.primary)
                    }
                    .accessibilityIdentifier("settings-notices")
                }

                Section {
                    Button("Lock now") { model.lock() }
                        .accessibilityIdentifier("settings-lock")
                    Button("Forget saved token", role: .destructive) { confirmingForget = true }
                        .accessibilityIdentifier("settings-forget")
                }
            }
            .hushListBackground()
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .confirmationDialog("Forget the saved token?", isPresented: $confirmingForget, titleVisibility: .visible) {
                Button("Forget", role: .destructive) { Task { await model.forget() } }
            } message: {
                Text("Hush locks and deletes the token from this device. Secrets in Bitwarden do not change.")
            }
        }
    }
}
