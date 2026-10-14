import SwiftUI
import UIKit

// Colors and shared chrome. Light is the default. Dark is true black with white primary text.
extension Color {
    /// Screen background. White in light mode, pure black in dark mode.
    static let hushBackground = Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? .black : .systemBackground })

    /// Accent for buttons and selection. Purple on white, cyan on black, both above 4.5:1 contrast.
    static let hushTint = Color(uiColor: UIColor {
        $0.userInterfaceStyle == .dark
            ? UIColor(red: 0x22 / 255, green: 0xD3 / 255, blue: 0xEE / 255, alpha: 1)
            : UIColor(red: 0x6D / 255, green: 0x28 / 255, blue: 0xD9 / 255, alpha: 1)
    })

    static let hushCyan = Color(red: 0x22 / 255, green: 0xD3 / 255, blue: 0xEE / 255)
    static let hushPurple = Color(red: 0x8B / 255, green: 0x5C / 255, blue: 0xF6 / 255)
}

/// The Hush mark and name. Used only on the connect, locked, and privacy screens.
struct HushWordmark: View {
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "lock.fill")
                .font(.title.weight(.semibold))
                .foregroundStyle(LinearGradient(colors: [.hushCyan, .hushPurple], startPoint: .topLeading, endPoint: .bottomTrailing))
                .accessibilityHidden(true)
            Text("Hush")
                .font(.largeTitle.bold())
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// Opaque cover shown whenever the scene is not active, including during a Face ID prompt.
struct PrivacyCover: View {
    var body: some View {
        ZStack {
            Color.hushBackground.ignoresSafeArea()
            HushWordmark()
        }
        .accessibilityIdentifier("privacy-cover")
    }
}

private struct PrivacyCovered: ViewModifier {
    @Environment(\.scenePhase) private var scenePhase

    func body(content: Content) -> some View {
        content.overlay {
            if scenePhase != .active { PrivacyCover() }
        }
    }
}

extension View {
    /// Covers this view while the scene is inactive or in the background.
    /// Apply to the root and to every sheet, because sheets sit above the root overlay.
    func privacyCovered() -> some View { modifier(PrivacyCovered()) }

    /// Plain list on the Hush background, so dark mode stays true black.
    func hushListBackground() -> some View {
        scrollContentBackground(.hidden).background(Color.hushBackground)
    }
}
