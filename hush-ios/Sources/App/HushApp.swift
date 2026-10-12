import SwiftUI

@main
struct HushApp: App {
    @State private var model: AppModel
    @AppStorage(AppearanceKey.darkMode) private var darkMode = false
    @Environment(\.scenePhase) private var scenePhase

    init() {
        _model = State(initialValue: AppModel(store: Self.makeStore()))
    }

    var body: some Scene {
        let root = RootView(model: model)
            .tint(.hushTint)
            .preferredColorScheme(darkMode ? .dark : .light)
            .privacyCovered()
            .task { await model.restore() }
        Self.makeWindow(root)
        .onChange(of: scenePhase) { _, phase in
            // Inactive only covers the screen, so a Face ID prompt never locks the app it is unlocking.
            if phase == .background { model.enterBackground() }
        }
    }

    // SwiftUI may invoke the window builder on its async renderer. It only returns a view built on
    // the main actor, so the builder must not inherit HushApp's main-actor isolation.
    nonisolated private static func makeWindow<Content: View>(_ content: Content) -> WindowGroup<Content> {
        WindowGroup { content }
    }

    private static func makeStore() -> VaultStore {
        #if DEBUG
        if let store = UITestHarness.store() { return store }
        #endif
        return .live()
    }
}

enum AppearanceKey {
    static let darkMode = "appearance.darkMode"
    static let region = "connection.region"
}
