import SwiftUI

/// An add awaiting a reply or with no confirmed result. The user refreshes the original vault,
/// reviews any new secret with the same key and project, and then marks it reviewed. Hush never sends the add again on its own.
struct PendingCreateSection: View {
    let model: AppModel
    let catalog: Catalog
    let attempt: CreateAttempt

    private var isOriginalVault: Bool { model.store.vaultIdentity == attempt.vaultIdentity }

    private var projectName: String {
        guard isOriginalVault else { return "the original project" }
        return catalog.project(attempt.projectID)?.name ?? "the original project"
    }

    var body: some View {
        Section {
            VStack(alignment: .leading, spacing: 4) {
                Label(
                    model.store.pendingCreateSettled ? "Unconfirmed add" : "Adding secret",
                    systemImage: model.store.pendingCreateSettled ? "questionmark.circle" : "arrow.up.circle"
                )
                .font(.headline)
                if model.store.pendingCreateSettled {
                    Text("Hush sent \(Text(attempt.key).monospaced()) to \(projectName) at \(attempt.startedAt, style: .time) without a confirmed result. It may or may not exist.")
                        .font(.callout)
                } else {
                    Text("Hush is waiting for a reply to the add of \(Text(attempt.key).monospaced()) to \(projectName).")
                        .font(.callout)
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("pending-summary")

            if !isOriginalVault {
                Text("This add belongs to a different vault. Connect the original organization in the \(attempt.vaultIdentity.region.rawValue.uppercased()) region to check it.")
                    .font(.callout)
                    .accessibilityIdentifier("pending-original-vault")
                Text(attempt.vaultIdentity.organizationID.uuidString)
                    .font(.caption.monospaced())
            } else if model.store.canAcknowledgePendingCreate {
                let candidates = attempt.candidates(in: catalog)
                if candidates.isEmpty {
                    Text("No new secret with this key and project after refresh.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("pending-no-candidates")
                } else {
                    ForEach(candidates) { candidate in
                        Button { model.selection = candidate.id } label: {
                            SecretRow(secret: candidate, catalog: catalog, showsID: true)
                        }
                        .accessibilityHint("Possible result of the unconfirmed add")
                        .accessibilityIdentifier("pending-candidate")
                    }
                }
                Button("Mark reviewed") { model.acknowledgePendingCreate() }
                    .accessibilityIdentifier("pending-acknowledge")
            } else if model.store.pendingCreateSettled {
                Button { Task { await model.refresh() } } label: {
                    Text(model.isRefreshing ? "Refreshing" : "Refresh to check")
                }
                .disabled(model.isRefreshing)
                .accessibilityIdentifier("pending-refresh")
            }
        }
    }
}
