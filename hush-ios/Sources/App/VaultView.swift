import SwiftUI

/// The unlocked vault. iPhone is a search-first stack. iPad is projects, secrets, and detail.
/// Both read the same selection, so a size class change keeps the open secret.
struct VaultView: View {
    @Bindable var model: AppModel
    let catalog: Catalog
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var columns: NavigationSplitViewVisibility = .all

    var body: some View {
        Group {
            if sizeClass == .compact {
                NavigationStack(path: path) {
                    SecretListView(model: model, catalog: catalog, selectsInPlace: false)
                        .navigationDestination(for: SecretID.self) { id in
                            SecretDetailView(model: model, catalog: catalog, id: id)
                        }
                }
            } else {
                NavigationSplitView(columnVisibility: $columns) {
                    ProjectSidebar(model: model, catalog: catalog)
                } content: {
                    SecretListView(model: model, catalog: catalog, selectsInPlace: true)
                } detail: {
                    NavigationStack {
                        if let id = model.selection {
                            SecretDetailView(model: model, catalog: catalog, id: id)
                        } else {
                            ContentUnavailableView("No secret selected", systemImage: "key")
                        }
                    }
                }
            }
        }
        .sheet(item: $model.sheet) { sheet in
            Group {
                switch sheet {
                case .create: CreateSecretView(model: model, catalog: catalog)
                case .settings: SettingsView(model: model)
                }
            }
            .privacyCovered()
        }
    }

    /// The iPhone stack path is the selection: empty, or the one open secret.
    private var path: Binding<[SecretID]> {
        Binding(
            get: { model.selection.map { [$0] } ?? [] },
            set: { model.selection = $0.last }
        )
    }
}

/// iPad sidebar. Picks the project filter.
private struct ProjectSidebar: View {
    @Bindable var model: AppModel
    let catalog: Catalog

    private enum Item: Hashable {
        case all
        case project(ProjectID)
    }

    private var selection: Binding<Item?> {
        Binding(
            get: { model.projectFilter.map(Item.project) ?? .all },
            set: { item in
                switch item {
                case .project(let id): model.projectFilter = id
                case .all, nil: model.projectFilter = nil
                }
            }
        )
    }

    var body: some View {
        List(selection: selection) {
            Label("All secrets", systemImage: "tray.full").tag(Item.all)
            ForEach(catalog.projects) { project in
                Label(project.name, systemImage: "folder")
                    .tag(Item.project(project.id))
                    .accessibilityIdentifier("sidebar-project")
            }
        }
        .navigationTitle("Hush")
    }
}
