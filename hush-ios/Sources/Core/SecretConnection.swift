import Foundation

/// One bridge session. `VaultStore` makes one per connect or unlock and closes it on lock.
/// Production is `SDKConnection`. Tests use a fake.
nonisolated protocol SecretConnection: AnyObject, Sendable {
    /// Sends one request and returns the raw reply bytes. Throws `CancellationError` when the
    /// calling task is cancelled, or `VaultFailure` when the session is closed or unusable.
    func send(_ request: BridgeRequest) async throws -> Data

    /// Ends the session after any call already running. Returns at once. Later sends fail.
    func close()
}

typealias ConnectionFactory = @Sendable (Region) -> any SecretConnection
