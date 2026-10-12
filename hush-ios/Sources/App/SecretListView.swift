import SwiftUI

/// Searchable secret metadata. Rows are keyed by secret ID, so two secrets with the same key
/// stay two rows and each opens its own detail.
struct SecretListView: View {
    @Bindable var model: AppModel
    let catalog: Catalog
    /// iPad selects rows in place for the detail column. iPhone pushes the detail.
    let selectsInPlace: Bool

    /// A filter whose project vanished after a refresh falls back to all projects.
    private var filter: ProjectID? {
        model.projectFilter.flatMap { catalog.project($0)?.id }
    }

    private var title: String {
        filter.flatMap { catalog.project($0)?.name } ?? (selectsInPlace ? "All secrets" : "Secrets")
    }

    var body: some View {
        let rows = catalog.matching(query: model.query, project: filter)
        let sharedKeys = Self.sharedKeys(in: catalog)
        list(rows: rows, sharedKeys: sharedKeys)
            .listStyle(.plain)
            .hushListBackground()
            .overlay { emptyState(rows: rows) }
            .navigationTitle(title)
            .searchable(
                text: $model.query,
                placement: selectsInPlace ? .automatic : .navigationBarDrawer(displayMode: .always),
                prompt: "Search secrets"
            )
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)
            .toolbar { toolbar(count: rows.count) }
    }

    @ViewBuilder
    private func list(rows: [SecretSummary], sharedKeys: Set<String>) -> some View {
        if selectsInPlace {
            List(selection: $model.selection) { sections(rows: rows, sharedKeys: sharedKeys) }
        } else {
            List { sections(rows: rows, sharedKeys: sharedKeys) }
        }
    }

    @ViewBuilder
    private func sections(rows: [SecretSummary], sharedKeys: Set<String>) -> some View {
        if let attempt = model.store.pendingCreate {
            PendingCreateSection(model: model, catalog: catalog, attempt: attempt)
        }
        if let failure = model.vaultFailure {
            Section { FailureMessage(failure: failure, context: .vault) }
        }
        Section {
            ForEach(rows) { secret in
                let row = SecretRow(secret: secret, catalog: catalog, showsID: sharedKeys.contains(secret.key))
                if selectsInPlace {
                    row.tag(secret.id)
                } else {
                    NavigationLink(value: secret.id) { row }
                }
            }
        }
    }

    @ViewBuilder
    private func emptyState(rows: [SecretSummary]) -> some View {
        if rows.isEmpty {
            if model.query.trimmingCharacters(in: .whitespaces).isEmpty {
                ContentUnavailableView("No secrets", systemImage: "key")
            } else {
                ContentUnavailableView.search(text: model.query)
            }
        }
    }

    @ToolbarContentBuilder
    private func toolbar(count: Int) -> some ToolbarContent {
        ToolbarItem(placement: .topBarLeading) {
            Button { model.lock() } label: { Label("Lock", systemImage: "lock") }
                .keyboardShortcut("l", modifiers: .command)
                .accessibilityIdentifier("lock-button")
        }
        if !selectsInPlace {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Picker("Project", selection: $model.projectFilter) {
                        Text("All projects").tag(ProjectID?.none)
                        ForEach(catalog.projects) { project in
                            Text(project.name).tag(Optional(project.id))
                        }
                    }
                } label: {
                    Label("Project", systemImage: filter == nil ? "line.3.horizontal.decrease.circle" : "line.3.horizontal.decrease.circle.fill")
                }
                .accessibilityIdentifier("project-filter")
            }
        }
        ToolbarItem(placement: .topBarTrailing) {
            Button { model.openCreate() } label: { Label("Add secret", systemImage: "plus") }
                .keyboardShortcut("n", modifiers: .command)
                .disabled(model.store.pendingCreate != nil)
                .accessibilityIdentifier("add-button")
        }
        ToolbarItemGroup(placement: .bottomBar) {
            Button { model.sheet = .settings } label: { Label("Settings", systemImage: "gearshape") }
                .keyboardShortcut(",", modifiers: .command)
                .accessibilityIdentifier("settings-button")
            Spacer()
            Text(model.isRefreshing ? "Refreshing" : "^[\(count) secret](inflect: true)")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
                .accessibilityIdentifier("list-status")
            Spacer()
            Button { Task { await model.refresh() } } label: { Label("Refresh", systemImage: "arrow.clockwise") }
                .keyboardShortcut("r", modifiers: .command)
                .disabled(model.isRefreshing)
                .accessibilityIdentifier("refresh-button")
        }
    }

    private static func sharedKeys(in catalog: Catalog) -> Set<String> {
        var seen = Set<String>()
        var shared = Set<String>()
        for secret in catalog.secrets where !seen.insert(secret.key).inserted {
            shared.insert(secret.key)
        }
        return shared
    }
}

/// One secret's metadata: key, projects, and a short ID when another secret has the same key.
struct SecretRow: View {
    let secret: SecretSummary
    let catalog: Catalog
    let showsID: Bool
    @Environment(\.dynamicTypeSize) private var typeSize

    private var projects: String {
        let names = secret.projectIDs.compactMap { catalog.project($0)?.name }
        return names.isEmpty ? "No project" : names.joined(separator: ", ")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(secret.key)
                .font(.body.monospaced())
                .lineLimit(typeSize.isAccessibilitySize ? nil : 1)
                .truncationMode(.middle)
            HStack(spacing: 8) {
                Text(projects)
                if showsID {
                    Text(secret.id.shortText)
                        .monospaced()
                        .accessibilityLabel("ID \(secret.id.shortText)")
                }
            }
            .font(.footnote)
            .foregroundStyle(.secondary)
            .lineLimit(typeSize.isAccessibilitySize ? nil : 1)
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("secret-row")
    }
}

extension SecretID {
    /// First eight hex digits. Enough to tell two same-key secrets apart on screen.
    var shortText: String { String(rawValue.uuidString.prefix(8)).lowercased() }
    var fullText: String { rawValue.uuidString.lowercased() }
}
