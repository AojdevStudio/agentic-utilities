import Foundation
import HushBridge
import os
import Testing
@testable import Hush

/// A stand-in for the four C entry points. It owns real heap buffers so the tests can prove
/// every reply is freed exactly once and the session is dropped after outstanding calls.
nonisolated final class FakeABI: Sendable {
    struct State: Sendable {
        var events: [String] = []
        var liveBuffers: Set<Int> = []
        var callsOnMainThread = 0
    }

    let state = OSAllocatedUnfairLock(initialState: State())
    let callStarted: AsyncStream<Void>
    private let callStartedSink: AsyncStream<Void>.Continuation
    private let release = DispatchSemaphore(value: 0)
    private let blocking: Bool
    private let reply: Data?
    private let session: Int?

    init(blocking: Bool = false, reply: Data? = Reply.catalog(Fixture.catalog), opens: Bool = true) {
        self.blocking = blocking
        self.reply = reply
        session = opens ? 0x5E55 : nil
        (callStarted, callStartedSink) = AsyncStream.makeStream()
    }

    func finishCall() { release.signal() }

    var events: [String] { state.withLock { $0.events } }

    var abi: BridgeABI {
        BridgeABI(
            open: { [self] _ in
                record("open")
                return session.flatMap { OpaquePointer(bitPattern: $0) }
            },
            call: { [self] _, _ in
                state.withLock {
                    $0.events.append("call")
                    if Thread.isMainThread { $0.callsOnMainThread += 1 }
                }
                callStartedSink.yield()
                if blocking { release.wait() }
                guard let reply else { return HushBuffer(data: nil, len: 0) }
                let bytes = UnsafeMutablePointer<UInt8>.allocate(capacity: reply.count)
                reply.copyBytes(to: bytes, count: reply.count)
                let address = Int(bitPattern: bytes)
                state.withLock { _ = $0.liveBuffers.insert(address) }
                return HushBuffer(data: bytes, len: reply.count)
            },
            drop: { [self] _ in record("drop") },
            free: { [self] buffer in
                record("free")
                guard let data = buffer.data else { return }
                let address = Int(bitPattern: data)
                let removed = state.withLock { $0.liveBuffers.remove(address) }
                precondition(removed != nil, "buffer freed twice or never allocated")
                data.deallocate()
            }
        )
    }

    private func record(_ event: String) { state.withLock { $0.events.append(event) } }
}

@MainActor @Suite(.timeLimit(.minutes(1))) struct SDKConnectionTests {
    @Test func callsRunOffTheMainThreadAndFreeTheReply() async throws {
        let expected = Reply.catalog(Fixture.catalog)
        let fake = FakeABI(reply: expected)
        let connection = SDKConnection(region: .eu, abi: fake.abi)

        let reply = try await connection.send(.catalog)

        #expect(reply == expected)
        #expect(fake.events == ["open", "call", "free"])
        #expect(fake.state.withLock { $0.callsOnMainThread } == 0)
        #expect(fake.state.withLock { $0.liveBuffers }.isEmpty)
    }

    @Test func emptyReplyIsStillFreed() async {
        let fake = FakeABI(reply: nil)
        let connection = SDKConnection(region: .us, abi: fake.abi)

        await #expect(throws: BridgeError.malformed) { try await connection.send(.catalog) }
        #expect(fake.events == ["open", "call", "free"])
    }

    @Test func failedOpenSendsNothing() async {
        let fake = FakeABI(opens: false)
        let connection = SDKConnection(region: .us, abi: fake.abi)

        await #expect(throws: VaultFailure.generic) { try await connection.send(.catalog) }
        #expect(fake.events == ["open"])
    }

    @Test func closeWaitsForTheOutstandingCallThenRefusesNewOnes() async throws {
        let fake = FakeABI(blocking: true)
        let connection = SDKConnection(region: .us, abi: fake.abi)

        let pending = Task { try await connection.send(.catalog) }
        _ = await fake.callStarted.next()
        connection.close()
        #expect(!fake.events.contains("drop"))

        fake.finishCall()
        _ = try await pending.value
        await #expect(throws: VaultFailure.locked) { try await connection.send(.catalog) }
        #expect(fake.events == ["open", "call", "free", "drop"])
    }

    @Test func cancelledCallerResumesOnceAndTheBufferIsFreedOnce() async throws {
        let fake = FakeABI(blocking: true)
        let connection = SDKConnection(region: .us, abi: fake.abi)

        let pending = Task { try await connection.send(.catalog) }
        _ = await fake.callStarted.next()
        pending.cancel()
        await #expect(throws: CancellationError.self) { try await pending.value }

        fake.finishCall()
        connection.close()
        await #expect(throws: VaultFailure.locked) { try await connection.send(.catalog) }
        #expect(fake.events == ["open", "call", "free", "drop"])
        #expect(fake.state.withLock { $0.liveBuffers }.isEmpty)
    }
}
