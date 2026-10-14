import SwiftUI
import UIKit

/// Where a failure happened. The same failure reads differently on the connect screen and in the vault.
enum FailureContext {
    case connect, unlock, vault
}

extension VaultFailure {
    /// Plain words for the user. Never includes raw bridge, SDK, or Keychain text.
    func message(in context: FailureContext) -> String {
        switch self {
        case .invalidInput:
            "Check the values and try again."
        case .invalidToken:
            context == .unlock
                ? "Bitwarden rejected the saved access token. Replace it, or forget it and connect again."
                : "Bitwarden did not accept this access token. Use a machine account access token from Secrets Manager."
        case .authentication:
            context == .vault
                ? "Bitwarden ended the session. Lock and unlock to sign in again."
                : "Sign in failed. The token may be expired or revoked, or it belongs to the other region."
        case .permission:
            "This machine account does not have access to that project or secret."
        case .network:
            "Hush could not reach Bitwarden. Check your connection and the region."
        case .locked:
            "Hush locked. Unlock to continue."
        case .notConfigured:
            "No access token is saved on this device. Connect again."
        case .deviceAuthenticationCancelled:
            "Unlock was cancelled."
        case .deviceAuthenticationUnavailable:
            "Hush needs a device passcode. Turn on a passcode in Settings, then try again."
        case .deviceAuthenticationFailed:
            "Face ID or passcode did not match. Try again."
        case .createPending:
            "An earlier add is unconfirmed. Refresh and review it before you add another secret."
        case .generic:
            "Something went wrong. Try again."
        }
    }

    /// True when the fix is in the Settings app, so the view offers a button to open it.
    var needsSystemSettings: Bool { self == .deviceAuthenticationUnavailable }
}

/// A failure line with an Open Settings button when the fix is a device passcode.
struct FailureMessage: View {
    let failure: VaultFailure
    let context: FailureContext
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(failure.message(in: context), systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red)
            if failure.needsSystemSettings, let url = URL(string: UIApplication.openSettingsURLString) {
                Button("Open Settings") { openURL(url) }
            }
        }
        .accessibilityIdentifier("failure-message")
    }
}

extension NewSecret.Invalid {
    var message: String {
        switch self {
        case .emptyKey: "Enter a key."
        case .emptyValue: "Enter a value."
        case .missingProject: "Choose a project."
        case .unknownProject: "That project is no longer available. Choose another."
        }
    }
}

extension Region {
    var name: String {
        switch self {
        case .us: "United States"
        case .eu: "European Union"
        }
    }

    var shortName: String {
        switch self {
        case .us: "US"
        case .eu: "EU"
        }
    }
}
