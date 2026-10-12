import SwiftUI

/// Add a secret: project, key, value, optional note. A failed save keeps the draft.
/// Save sends once. There is no automatic retry.
struct CreateSecretView: View {
    @Bindable var model: AppModel
    let catalog: Catalog
    @FocusState private var focus: Field?

    private enum Field { case key, value, note }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Project", selection: $model.draft.projectID) {
                        Text("Choose").tag(ProjectID?.none)
                        ForEach(catalog.projects) { project in
                            Text(project.name).tag(Optional(project.id))
                        }
                    }
                    .accessibilityIdentifier("draft-project")
                    fieldError(for: [.missingProject, .unknownProject])

                    TextField("Key", text: $model.draft.key)
                        .font(.body.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .focused($focus, equals: .key)
                        .submitLabel(.next)
                        .onSubmit { focus = .value }
                        .accessibilityIdentifier("draft-key")
                    fieldError(for: [.emptyKey])

                    SecureField("Value", text: $model.draft.value)
                        .font(.body.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .textContentType(.none)
                        .focused($focus, equals: .value)
                        .submitLabel(.next)
                        .onSubmit { focus = .note }
                        .accessibilityIdentifier("draft-value")
                    fieldError(for: [.emptyValue])

                    TextField("Note (optional)", text: $model.draft.note, axis: .vertical)
                        .lineLimit(2...6)
                        .autocorrectionDisabled()
                        .focused($focus, equals: .note)
                        .privacySensitive()
                        .accessibilityIdentifier("draft-note")
                }
                .disabled(model.isSaving)

                if let failure = model.draft.failure {
                    Section {
                        FailureMessage(failure: failure, context: .vault)
                        Text("Your draft is kept. Nothing was saved.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .hushListBackground()
            .navigationTitle("Add secret")
            .navigationBarTitleDisplayMode(.inline)
            .interactiveDismissDisabled(model.isSaving)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { model.cancelCreate() }
                        .disabled(model.isSaving)
                        .accessibilityIdentifier("draft-cancel")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(model.isSaving ? "Saving" : "Save") { Task { await model.saveDraft() } }
                        .disabled(model.isSaving)
                        .keyboardShortcut("s", modifiers: .command)
                        .accessibilityIdentifier("draft-save")
                }
            }
            .onAppear { focus = model.draft.projectID == nil ? nil : .key }
        }
    }

    @ViewBuilder
    private func fieldError(for cases: Set<NewSecret.Invalid>) -> some View {
        if let invalid = model.draft.invalid, cases.contains(invalid) {
            Text(invalid.message)
                .font(.footnote)
                .foregroundStyle(.red)
                .accessibilityIdentifier("draft-error")
        }
    }
}
