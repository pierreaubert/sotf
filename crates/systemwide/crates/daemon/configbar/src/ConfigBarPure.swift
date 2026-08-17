import Foundation

/// Shared decision logic for startup adoption. A reachable socket belongs to
/// the daemon already listening on it; the Configbar must not replace that
/// process unless it is the process it launched itself.
enum ConfigBarDaemonAdoption {
    static func shouldAdopt(
        reachable: Bool,
        managedProcessRunning: Bool
    ) -> Bool {
        reachable && !managedProcessRunning
    }
}

/// Dispatch a potentially blocking daemon operation away from SwiftUI's main
/// thread and deliver its result back on the main queue.
enum ConfigBarAsyncOperation {
    static func perform<Result>(
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

func isConfigBarVirtualDevice(_ name: String) -> Bool {
    configBarVirtualDevicePatterns.contains { pattern in
        name.range(of: pattern, options: [.caseInsensitive, .diacriticInsensitive]) != nil
    }
}

func sanitizeConfigBarPeaks(_ peaks: [Double], maxChannels: Int = 32) -> [Double] {
    let limit = min(max(maxChannels, 1), 32)
    return peaks.prefix(limit).map { peak in
        guard peak.isFinite, peak > 0 else { return 0.0 }
        return min(peak, 2.0)
    }
}

func decayConfigBarPeaks(_ peaks: [Double], factor: Double = 0.85) -> [Double] {
    peaks.map { peak in
        let next = max(peak, 0.0) * factor
        return next < 0.00001 ? 0.0 : next
    }
}

func updateConfigBarPeakHolds(previous: [Double], current: [Double]) -> [Double] {
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
struct EncryptionToggleGuard {
    private var ignoreNextChange = false

    mutating func markProgrammaticChange() {
        ignoreNextChange = true
    }

    mutating func consumeProgrammaticChange() -> Bool {
        guard ignoreNextChange else { return false }
        ignoreNextChange = false
        return true
    }
}
