// Timing.swift - Clock and timing utilities for SotF HAL Driver

import Foundation

/// Manages the driver's clock for synchronization with Core Audio
final class DriverClock {
    private let lock = NSLock()
    private var anchorHostTime: UInt64 = 0
    private var anchorSampleTime: Float64 = 0
    private var sampleRate: Float64 = 48000.0
    private var clockSeed: UInt64 = 1
    private var isRunning: Bool = false

    private static var timebaseInfo: mach_timebase_info_data_t = {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return info
    }()

    /// Reset the clock anchor when IO starts
    func start(sampleRate: Float64) {
        lock.lock()
        defer { lock.unlock() }

        self.sampleRate = sampleRate
        anchorHostTime = mach_absolute_time()
        anchorSampleTime = 0
        clockSeed += 1
        isRunning = true
    }

    /// Stop the clock
    func stop() {
        lock.lock()
        defer { lock.unlock() }

        isRunning = false
    }

    /// Update sample rate (triggers clock seed change)
    func setSampleRate(_ rate: Float64) {
        lock.lock()
        defer { lock.unlock() }

        if rate != sampleRate {
            let currentHostTime = mach_absolute_time()
            anchorSampleTime = getCurrentSampleTimeLocked(at: currentHostTime)
            sampleRate = rate
            anchorHostTime = currentHostTime
            clockSeed += 1
        }
    }

    /// Convert host time to nanoseconds
    private func hostTimeToNanoseconds(_ hostTime: UInt64) -> UInt64 {
        return hostTime * UInt64(Self.timebaseInfo.numer) / UInt64(Self.timebaseInfo.denom)
    }

    /// Get current sample time for a given host time
    private func getCurrentSampleTimeLocked(at hostTime: UInt64) -> Float64 {
        guard hostTime >= anchorHostTime else { return anchorSampleTime }

        let hostTimeDelta = hostTime - anchorHostTime
        let nanoseconds = hostTimeToNanoseconds(hostTimeDelta)
        let seconds = Double(nanoseconds) / 1_000_000_000.0
        let samplesDelta = seconds * sampleRate

        return anchorSampleTime + samplesDelta
    }

    /// Get the zero timestamp for Core Audio synchronization
    /// Returns (sampleTime, hostTime, seed)
    func getZeroTimeStamp(period: UInt32) -> (Float64, UInt64, UInt64) {
        lock.lock()
        defer { lock.unlock() }

        let currentHostTime = mach_absolute_time()
        let currentSampleTime = getCurrentSampleTimeLocked(at: currentHostTime)

        // Zero timestamps advance by kAudioDevicePropertyZeroTimeStampPeriod,
        // not by the IO buffer size.
        let periodFrames = Float64(max(period, 1))
        let zeroSampleTime = floor(currentSampleTime / periodFrames) * periodFrames

        // Calculate the host time that corresponds to the zero sample time
        let sampleOffset = zeroSampleTime - anchorSampleTime
        let secondsOffset = sampleOffset / sampleRate
        let nanosOffset = UInt64(secondsOffset * 1_000_000_000.0)
        let hostOffset = nanosOffset * UInt64(Self.timebaseInfo.denom) / UInt64(Self.timebaseInfo.numer)
        let zeroHostTime = anchorHostTime + hostOffset

        return (zeroSampleTime, zeroHostTime, clockSeed)
    }

    /// Get current sample time
    func getCurrentSampleTime() -> Float64 {
        lock.lock()
        defer { lock.unlock() }

        return getCurrentSampleTimeLocked(at: mach_absolute_time())
    }

    /// Get the clock seed (changes when timing is reset)
    func getSeed() -> UInt64 {
        lock.lock()
        defer { lock.unlock() }

        return clockSeed
    }

    var running: Bool {
        lock.lock()
        defer { lock.unlock() }

        return isRunning
    }
}
