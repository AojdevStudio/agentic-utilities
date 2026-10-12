import SwiftUI

/// First run, and replacing a saved token. Takes a machine account access token and a region.
struct ConnectView: View {
    @Bindable var model: AppModel
    @AppStorage(AppearanceKey.region) private var savedRegion: Region = .us
    @State private var region: Region = .us
    @FocusState private var tokenFocused: Bool

    private var canConnect: Bool {
        !model.isConnecting && !model.tokenDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        HushWordmark()
                        Text(model.isReplacingToken ? "Replace the saved access token." : "Connect to Bitwarden Secrets Manager.")
                            .foregroundStyle(.secondary)
                    }
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }

                Section {
                    SecureField("Access token", text: $model.tokenDraft)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textContentType(.none)
                        .font(.body.monospaced())
                        .focused($tokenFocused)
                        .submitLabel(.go)
                        .onSubmit(connect)
                        .disabled(model.isConnecting)
                        .accessibilityIdentifier("token-field")
                    Picker("Region", selection: $region) {
                        ForEach(Region.allCases, id: \.self) { region in
                            Text(region.name).tag(region)
                        }
                    }
                    .disabled(model.isConnecting)
                    .accessibilityIdentifier("region-picker")
                } footer: {
                    Text("Use a machine account access token from Secrets Manager, not your password or a personal API key. Hush keeps it in this device's Keychain behind Face ID or your passcode.")
                }

                Section {
                    Button(action: connect) {
                        Text(model.isConnecting ? "Connecting" : "Connect")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!canConnect)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                    .accessibilityIdentifier("connect-button")
                }

                if let failure = model.connectFailure {
                    Section { FailureMessage(failure: failure, context: .connect) }
                }

                if let attempt = model.store.pendingCreate {
                    Section { PendingCreateNotice(attempt: attempt) }
                }
            }
            .hushListBackground()
            .toolbar {
                if model.isReplacingToken {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { model.cancelReplacingToken() }
                            .disabled(model.isConnecting)
                    }
                }
            }
        }
        .onAppear {
            region = savedRegion
            tokenFocused = true
        }
    }

    private func connect() {
        guard canConnect else { return }
        let chosen = region
        Task {
            if await model.connect(region: chosen) { savedRegion = chosen }
        }
    }
}
