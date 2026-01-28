// RingBuffer.swift - Lock-free ring buffer for audio data

import Foundation

/// A lock-free single-producer single-consumer ring buffer for audio samples
final class AudioRingBuffer {
    private let buffer: UnsafeMutablePointer<Float>
    private let capacity: Int
    private var writePosition: UInt64 = 0
    private var readPosition: UInt64 = 0

    /// Initialize with capacity in samples (not frames)
    init(capacity: Int) {
        self.capacity = capacity
        self.buffer = UnsafeMutablePointer<Float>.allocate(capacity: capacity)
        self.buffer.initialize(repeating: 0, count: capacity)
    }

    deinit {
        buffer.deallocate()
    }

    /// Reset the buffer to empty state
    func reset() {
        writePosition = 0
        readPosition = 0
        buffer.initialize(repeating: 0, count: capacity)
    }

    /// Number of samples available to read
    var availableToRead: Int {
        let write = writePosition
        let read = readPosition
        return Int(write - read)
    }

    /// Number of samples available to write
    var availableToWrite: Int {
        return capacity - availableToRead
    }

    /// Write samples to the buffer
    /// Returns number of samples actually written
    @discardableResult
    func write(_ samples: UnsafePointer<Float>, count: Int) -> Int {
        let available = availableToWrite
        let toWrite = min(count, available)

        if toWrite == 0 { return 0 }

        let writeIndex = Int(writePosition % UInt64(capacity))
        let firstPart = min(toWrite, capacity - writeIndex)
        let secondPart = toWrite - firstPart

        // Copy first part (from writeIndex to end of buffer or toWrite)
        memcpy(buffer.advanced(by: writeIndex), samples, firstPart * MemoryLayout<Float>.size)

        // Copy second part (wrap around to beginning)
        if secondPart > 0 {
            memcpy(buffer, samples.advanced(by: firstPart), secondPart * MemoryLayout<Float>.size)
        }

        // Memory barrier before updating position
        OSMemoryBarrier()
        writePosition += UInt64(toWrite)

        return toWrite
    }

    /// Read samples from the buffer
    /// Returns number of samples actually read
    @discardableResult
    func read(_ samples: UnsafeMutablePointer<Float>, count: Int) -> Int {
        let available = availableToRead
        let toRead = min(count, available)

        if toRead == 0 {
            // Fill with silence if nothing available
            memset(samples, 0, count * MemoryLayout<Float>.size)
            return 0
        }

        let readIndex = Int(readPosition % UInt64(capacity))
        let firstPart = min(toRead, capacity - readIndex)
        let secondPart = toRead - firstPart

        // Copy first part
        memcpy(samples, buffer.advanced(by: readIndex), firstPart * MemoryLayout<Float>.size)

        // Copy second part (wrap around)
        if secondPart > 0 {
            memcpy(samples.advanced(by: firstPart), buffer, secondPart * MemoryLayout<Float>.size)
        }

        // Memory barrier before updating position
        OSMemoryBarrier()
        readPosition += UInt64(toRead)

        // Fill remaining with silence if we didn't read enough
        if toRead < count {
            memset(samples.advanced(by: toRead), 0, (count - toRead) * MemoryLayout<Float>.size)
        }

        return toRead
    }

    /// Peek at samples without advancing read position
    func peek(_ samples: UnsafeMutablePointer<Float>, count: Int) -> Int {
        let available = availableToRead
        let toPeek = min(count, available)

        if toPeek == 0 { return 0 }

        let readIndex = Int(readPosition % UInt64(capacity))
        let firstPart = min(toPeek, capacity - readIndex)
        let secondPart = toPeek - firstPart

        memcpy(samples, buffer.advanced(by: readIndex), firstPart * MemoryLayout<Float>.size)
        if secondPart > 0 {
            memcpy(samples.advanced(by: firstPart), buffer, secondPart * MemoryLayout<Float>.size)
        }

        return toPeek
    }

    /// Skip samples (advance read position without reading)
    func skip(_ count: Int) {
        let available = availableToRead
        let toSkip = min(count, available)
        OSMemoryBarrier()
        readPosition += UInt64(toSkip)
    }
}

/// Multi-channel audio ring buffer
final class MultiChannelRingBuffer {
    private let channelBuffers: [AudioRingBuffer]
    let channelCount: Int

    init(channelCount: Int, framesCapacity: Int) {
        self.channelCount = channelCount
        self.channelBuffers = (0..<channelCount).map { _ in
            AudioRingBuffer(capacity: framesCapacity)
        }
    }

    func reset() {
        channelBuffers.forEach { $0.reset() }
    }

    var availableFramesToRead: Int {
        channelBuffers.first?.availableToRead ?? 0
    }

    var availableFramesToWrite: Int {
        channelBuffers.first?.availableToWrite ?? 0
    }

    /// Write interleaved audio data
    func writeInterleaved(_ samples: UnsafePointer<Float>, frameCount: Int) -> Int {
        let available = availableFramesToWrite
        let toWrite = min(frameCount, available)

        if toWrite == 0 { return 0 }

        // Deinterleave and write to each channel
        for frame in 0..<toWrite {
            for channel in 0..<channelCount {
                var sample = samples[frame * channelCount + channel]
                channelBuffers[channel].write(&sample, count: 1)
            }
        }

        return toWrite
    }

    /// Read to interleaved audio data
    func readInterleaved(_ samples: UnsafeMutablePointer<Float>, frameCount: Int) -> Int {
        let available = availableFramesToRead
        let toRead = min(frameCount, available)

        if toRead == 0 {
            memset(samples, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

        // Read from each channel and interleave
        for frame in 0..<toRead {
            for channel in 0..<channelCount {
                var sample: Float = 0
                channelBuffers[channel].read(&sample, count: 1)
                samples[frame * channelCount + channel] = sample
            }
        }

        // Fill remaining with silence
        if toRead < frameCount {
            let remaining = (frameCount - toRead) * channelCount
            memset(samples.advanced(by: toRead * channelCount), 0, remaining * MemoryLayout<Float>.size)
        }

        return toRead
    }

    /// Write non-interleaved (planar) audio data
    func writeNonInterleaved(_ buffers: [UnsafePointer<Float>], frameCount: Int) -> Int {
        guard buffers.count == channelCount else { return 0 }

        let available = availableFramesToWrite
        let toWrite = min(frameCount, available)

        if toWrite == 0 { return 0 }

        for (channel, buffer) in buffers.enumerated() {
            channelBuffers[channel].write(buffer, count: toWrite)
        }

        return toWrite
    }

    /// Read to non-interleaved (planar) audio data
    func readNonInterleaved(_ buffers: [UnsafeMutablePointer<Float>], frameCount: Int) -> Int {
        guard buffers.count == channelCount else { return 0 }

        let available = availableFramesToRead
        let toRead = min(frameCount, available)

        for (channel, buffer) in buffers.enumerated() {
            if toRead > 0 {
                channelBuffers[channel].read(buffer, count: toRead)
            }
            // Fill remaining with silence
            if toRead < frameCount {
                memset(buffer.advanced(by: toRead), 0, (frameCount - toRead) * MemoryLayout<Float>.size)
            }
        }

        return toRead
    }
}
