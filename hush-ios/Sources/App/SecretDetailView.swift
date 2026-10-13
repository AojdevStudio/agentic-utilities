import SwiftUI

/// One secret, fetched when opened. The value stays masked until revealed. Copy is local to this
/// device and expires after 30 seconds.
struct SecretDetailView: View {
    @Bindable var model: AppModel
    let catalog: Catalog
    let id: SecretID

    var body: some View {
        content
            .hushListBackground()
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .task(id: id) { await model.loadDetail(id) }
    }

    private var title: String {
        if case .loaded(let detail) = model.detail, detail.id == id { return detail.key }
        return catalog.secrets.first { $0.id == id }?.key ?? "Secret"
    }

    @ViewBuilder
    private var content: some View {
        switch model.detail {
        case .loaded(let detail) where detail.id == id:
            DetailFields(model: model, catalog: catalog, detail: detail)
        case .failed(let failedID, let failure) where failedID == id:
            List {
                FailureMessage(failure: failure, context: .vault)
                Button("Try again") { Task { await model.retryDetail() } }
            }
        default:
            List {
                Text("Loading")
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("detail-loading")
            }
        }
    }
}

private struct DetailFields: View {
    @Bindable var model: AppModel
    let catalog: Catalog
    let detail: SecretDetail

    private var projectName: String {
        detail.projectID.flatMap { catalog.project($0)?.name } ?? "No project"
    }

    var body: some View {
        List {
            Section {
                LabeledContent("Key") {
                    Text(detail.key)
                        .font(.body.monospaced())
                        .textSelection(.enabled)
                        .accessibilityIdentifier("detail-key")
                }
                LabeledContent("Project", value: projectName)
                LabeledContent("ID") {
                    Text(detail.id.fullText)
                        .font(.footnote.monospaced())
                        .textSelection(.enabled)
                        .accessibilityIdentifier("detail-id")
                }
            }

            Section {
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text("Value")
                        Spacer()
                        Button {
                            model.isRevealed.toggle()
                        } label: {
                            Label(model.isRevealed ? "Hide" : "Reveal", systemImage: model.isRevealed ? "eye.slash" : "eye")
                        }
                        .buttonStyle(.borderless)
                        .keyboardShortcut("r", modifiers: [.command, .shift])
                        .accessibilityIdentifier("reveal-button")
                        Button {
                            model.copyValue()
                        } label: {
                            Label("Copy", systemImage: "doc.on.doc")
                        }
                        .buttonStyle(.borderless)
                        .keyboardShortcut("c", modifiers: [.command, .shift])
                        .padding(.leading, 12)
                        .accessibilityIdentifier("copy-button")
                    }
                    .labelStyle(.titleAndIcon)
                    valueText
                    if model.copiedSecret == detail.id {
                        Text("Copied. Clears in 30 seconds.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("copied-status")
                    }
                }
            }

            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Note")
                    if detail.note.isEmpty {
                        Text("No note").foregroundStyle(.secondary)
                    } else {
                        Text(detail.note)
                            .textSelection(.enabled)
                            .privacySensitive()
                            .accessibilityIdentifier("detail-note")
                    }
                }
            }
        }
    }

    /// The masked form never contains the value, not even its length.
    @ViewBuilder
    private var valueText: some View {
        if model.isRevealed {
            Text(detail.value)
                .font(.body.monospaced())
                .textSelection(.disabled)
                .privacySensitive()
                .accessibilityIdentifier("detail-value")
        } else {
            Text(verbatim: "••••••••••••")
                .font(.body.monospaced())
                .accessibilityLabel("Value hidden")
                .accessibilityIdentifier("detail-value-masked")
        }
    }
}
