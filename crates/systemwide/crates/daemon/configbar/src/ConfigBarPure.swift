import Foundation

/// Shared decision logic for startup adoption. A reachable socket belongs to
/// the daemon already listening on it; the Configbar must not replace that
/// process unless it is the process it launched itself.
public enum ConfigBarDaemonAdoption {
    public static func shouldAdopt(
        reachable: Bool,
        managedProcessRunning: Bool
    ) -> Bool {
        reachable && !managedProcessRunning
    }
}

/// Dispatch a potentially blocking daemon operation away from SwiftUI's main
/// thread and deliver its result back on the main queue.
public enum ConfigBarAsyncOperation {
    public static func perform<Result>(
        on queue: DispatchQueue,
        work: @escaping () -> Result,
        completion: @escaping (Result) -> Void
    ) {
        queue.async {
            let result = work()
            DispatchQueue.main.async {
                completion(result)
            }
        }
    }
}

/// Result of an optimistic Configbar mutation.  A stale completion is
/// represented by `nil` from `ConfigBarMutationState.resolve`, so an older
/// daemon response cannot roll back a newer user selection.
public enum ConfigBarMutationResult<Value: Equatable>: Equatable {
    case confirmed(Value)
    case rolledBack(Value)
}

/// Small, UI-independent state machine shared by the Configbar mutation
/// tests.  The production view keeps its SwiftUI `@State` values separate,
/// but must obey these same invariants: one generation per request, only the
/// current generation may commit, and a rejection restores the last value
/// confirmed by the daemon.
public struct ConfigBarMutationState<Value: Equatable> {
    public private(set) var confirmed: Value
    private var generation: UInt64 = 0

    public init(confirmed: Value) {
        self.confirmed = confirmed
    }

    @discardableResult
    public mutating func begin(_ requested: Value) -> UInt64 {
        generation &+= 1
        return generation
    }

    public mutating func resolve(
        generation: UInt64,
        requested: Value,
        succeeded: Bool
    ) -> ConfigBarMutationResult<Value>? {
        guard generation == self.generation else { return nil }
        if succeeded {
            confirmed = requested
            return .confirmed(requested)
        }
        return .rolledBack(confirmed)
    }
}

/// Watermark for asynchronous status snapshots. A snapshot is allowed to
/// reconcile mutable UI state only when it was requested after the latest
/// user mutation began. This keeps a slow status/device/config response from
/// overwriting a newer optimistic value.
public struct ConfigBarStatusWatermark: Equatable {
    public private(set) var generation: UInt64 = 0

    public init() {}

    @discardableResult
    public mutating func beginMutation() -> UInt64 {
        generation &+= 1
        return generation
    }

    public func accepts(snapshotGeneration: UInt64) -> Bool {
        snapshotGeneration == generation
    }
}

let configBarVirtualDevicePatterns = [
    "SotF",
    "BlackHole",
    "Loopback",
    "Virtual",
    "Soundflower",
    "Background Music",
    "Audio Bridge",
    "ZoomAudio",
]

public func isConfigBarVirtualDevice(_ name: String) -> Bool {
    configBarVirtualDevicePatterns.contains { pattern in
        name.range(of: pattern, options: [.caseInsensitive, .diacriticInsensitive]) != nil
    }
}

public func sanitizeConfigBarPeaks(_ peaks: [Double], maxChannels: Int = 32) -> [Double] {
    let limit = min(max(maxChannels, 1), 32)
    return peaks.prefix(limit).map { peak in
        guard peak.isFinite, peak > 0 else { return 0.0 }
        return min(peak, 2.0)
    }
}

public func decayConfigBarPeaks(_ peaks: [Double], factor: Double = 0.85) -> [Double] {
    peaks.map { peak in
        let next = max(peak, 0.0) * factor
        return next < 0.00001 ? 0.0 : next
    }
}

public func updateConfigBarPeakHolds(previous: [Double], current: [Double]) -> [Double] {
    current.enumerated().map { index, peak in
        let oldValue = index < previous.count ? previous[index] : 0.0
        if peak >= oldValue {
            return peak
        }
        let decayed = oldValue * 0.96
        return max(peak, decayed < 0.00001 ? 0.0 : decayed)
    }
}

/// Prevent a failed Toggle mutation from triggering a second daemon request
/// when SwiftUI observes the programmatic rollback.
public struct EncryptionToggleGuard {
    private var ignoreNextChange = false

    public init() {}

    public mutating func markProgrammaticChange() {
        ignoreNextChange = true
    }

    public mutating func consumeProgrammaticChange() -> Bool {
        guard ignoreNextChange else { return false }
        ignoreNextChange = false
        return true
    }
}

/// Pure window-dismissal policy used by the AppKit window subclass. Keeping
/// the decision outside AppKit makes the accessory-app lifecycle behavior
/// testable without creating a live NSWindow in a test process.
public enum ConfigBarWindowPolicy {
    public static func shouldDismissCommandW(
        hasCommandModifier: Bool,
        charactersIgnoringModifiers: String?
    ) -> Bool {
        hasCommandModifier && charactersIgnoringModifiers == "w"
    }
}
