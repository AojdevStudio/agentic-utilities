import Foundation
import HushBridge
import os

/// The four C entry points. Tests swap in closures that count frees and drops.
nonisolated struct BridgeABI: Sendable {
    var open: @Sendable (UInt32) -> OpaquePointer?
    var call: @Sendable (OpaquePointer, UnsafeRawBufferPointer) -> HushBuffer
    var drop: @Sendable (OpaquePointer) -> Void
    var free: @Sendable (HushBuffer) -> Void

    static let live = BridgeABI(
        open: { hush_session_new($0) },
        call: { session, bytes in
            hush_session_call(session, bytes.baseAddress?.assumingMemoryBound(to: UInt8.self), bytes.count)
        },
        drop: { hush_session_drop($0) },
        free: { hush_buffer_free($0) }
    )
}

/// One bridge session. All C calls, including the final drop, run in order on one private
/// serial queue, never on the main thread or a Swift concurrency thread.
/// `@unchecked Sendable`: `session` and `closed` are touched only on `queue`.
nonisolated final class SDKConnection: SecretConnection, @unchecked Sendable {
    private let region: Region
    private let abi: BridgeABI
    private let queue = DispatchQueue(label: "Hush.SDKConnection")
    private var session: OpaquePointer?
    private var closed = false

    init(region: Region, abi: BridgeABI = .live) {
        self.region = region
        self.abi = abi
    }

    deinit {
        // Every queued block retains self, so nothing is running on the queue here.
        guard let session else { return }
        nonisolated(unsafe) let handle = session
        let drop = abi.drop
        queue.async { drop(handle) }
    }

    /// A cancelled caller resumes at once. A C call already running still finishes and frees on the queue.
    func send(_ request: BridgeRequest) async throws -> Data {
        let payload = try BridgeWire.encode(request)
        let reply = OneShot<Data>()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                reply.attach(continuation)
                queue.async { [self] in
                    guard !reply.isFinished else { return }
                    reply.resume(with: perform(payload))
                }
            }
        } onCancel: {
            reply.resume(with: .failure(CancellationError()))
        }
    }

    func close() {
        queue.async { [self] in
            closed = true
            guard let session else { return }
            self.session = nil
            abi.drop(session)
        }
    }

    private func perform(_ payload: Data) -> Result<Data, any Error> {
        dispatchPrecondition(condition: .onQueue(queue))
        guard !closed else { return .failure(VaultFailure.locked) }
        if session == nil { session = abi.open(region.bridgeCode) }
        guard let session else { return .failure(VaultFailure.generic) }
        let buffer = payload.withUnsafeBytes { abi.call(session, $0) }
        defer { abi.free(buffer) }
        guard let bytes = buffer.data, buffer.len > 0 else { return .failure(BridgeError.malformed) }
        return .success(Data(bytes: bytes, count: buffer.len))
    }
}

/// Resumes one continuation exactly once, whichever of the reply or the cancellation comes first.
nonisolated private final class OneShot<Value: Sendable>: Sendable {
    private enum State: Sendable {
        case waiting
        case attached(CheckedContinuation<Value, any Error>)
        case early(Result<Value, any Error>)
        case finished
    }

    private let state = OSAllocatedUnfairLock(initialState: State.waiting)

    var isFinished: Bool {
        state.withLock { if case .finished = $0 { true } else { false } }
    }

    func attach(_ continuation: CheckedContinuation<Value, any Error>) {
        let early: Result<Value, any Error>? = state.withLock { state in
            guard case .early(let result) = state else {
                state = .attached(continuation)
                return nil
            }
            state = .finished
            return result
        }
        if let early { continuation.resume(with: early) }
    }

    func resume(with result: Result<Value, any Error>) {
        let continuation: CheckedContinuation<Value, any Error>? = state.withLock { state in
            switch state {
            case .waiting:
                state = .early(result)
                return nil
            case .attached(let continuation):
                state = .finished
                return continuation
            case .early, .finished:
                return nil
            }
        }
        continuation?.resume(with: result)
    }
}

extension VaultStore {
    static func live() -> VaultStore {
        VaultStore(
            credentials: KeychainCredentialStore(),
            deviceAuthenticator: LocalDeviceAuthenticator(),
            makeConnection: { SDKConnection(region: $0) }
        )
    }
}
