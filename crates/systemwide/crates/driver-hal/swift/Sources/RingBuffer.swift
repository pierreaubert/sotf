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
        sotf_atomic_store_u64(&writePosition, 0)
        sotf_atomic_store_u64(&readPosition, 0)
        buffer.initialize(repeating: 0, count: capacity)
    }

    /// Number of samples available to read
    ///
    /// Positions use C11 acquire/release atomics for SPSC publication.
    var availableToRead: Int {
        let write = sotf_atomic_load_u64(&writePosition)
        let read = sotf_atomic_load_u64(&readPosition)
        guard write >= read else { return 0 }
        return Int(min(UInt64(capacity), write - read))
    }

    /// Number of samples available to write
    ///
    /// Called from the producer side of the SPSC ring.
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

        let write = sotf_atomic_load_u64(&writePosition)
        let writeIndex = Int(write % UInt64(capacity))
        let firstPart = min(toWrite, capacity - writeIndex)
        let secondPart = toWrite - firstPart

        // Copy first part (from writeIndex to end of buffer or toWrite)
        memcpy(buffer.advanced(by: writeIndex), samples, firstPart * MemoryLayout<Float>.size)

        // Copy second part (wrap around to beginning)
        if secondPart > 0 {
            memcpy(buffer, samples.advanced(by: firstPart), secondPart * MemoryLayout<Float>.size)
        }

        // Memory barrier before updating position
        sotf_atomic_store_u64(&writePosition, write + UInt64(toWrite))

        return toWrite
    }

    /// Write one channel from an interleaved source, publishing the write
    /// position once for the whole block instead of once per sample.
    @discardableResult
    fileprivate func writeStrided(
        _ samples: UnsafePointer<Float>,
        count: Int,
        stride: Int
    ) -> Int {
        let available = availableToWrite
        let toWrite = min(count, available)
        guard toWrite > 0 else { return 0 }

        let write = sotf_atomic_load_u64(&writePosition)
        let writeIndex = Int(write % UInt64(capacity))
        let firstPart = min(toWrite, capacity - writeIndex)
        for index in 0..<firstPart {
            buffer[writeIndex + index] = samples[index * stride]
        }
        let secondPart = toWrite - firstPart
        for index in 0..<secondPart {
            buffer[index] = samples[(firstPart + index) * stride]
        }

        sotf_atomic_store_u64(&writePosition, write + UInt64(toWrite))
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

        let read = sotf_atomic_load_u64(&readPosition)
        let readIndex = Int(read % UInt64(capacity))
        let firstPart = min(toRead, capacity - readIndex)
        let secondPart = toRead - firstPart

        // Copy first part
        memcpy(samples, buffer.advanced(by: readIndex), firstPart * MemoryLayout<Float>.size)

        // Copy second part (wrap around)
        if secondPart > 0 {
            memcpy(samples.advanced(by: firstPart), buffer, secondPart * MemoryLayout<Float>.size)
        }

        // Memory barrier before updating position
        sotf_atomic_store_u64(&readPosition, read + UInt64(toRead))

        // Fill remaining with silence if we didn't read enough
        if toRead < count {
            memset(samples.advanced(by: toRead), 0, (count - toRead) * MemoryLayout<Float>.size)
        }

        return toRead
    }

    /// Read one channel into an interleaved destination, publishing the read
    /// position once for the whole block instead of once per sample.
    @discardableResult
    fileprivate func readStrided(
        _ samples: UnsafeMutablePointer<Float>,
        count: Int,
        stride: Int
    ) -> Int {
        let available = availableToRead
        let toRead = min(count, available)
        guard toRead > 0 else { return 0 }

        let read = sotf_atomic_load_u64(&readPosition)
        let readIndex = Int(read % UInt64(capacity))
        let firstPart = min(toRead, capacity - readIndex)
        for index in 0..<firstPart {
            samples[index * stride] = buffer[readIndex + index]
        }
        let secondPart = toRead - firstPart
        for index in 0..<secondPart {
            samples[(firstPart + index) * stride] = buffer[index]
        }

        sotf_atomic_store_u64(&readPosition, read + UInt64(toRead))
        return toRead
    }

    /// Peek at samples without advancing read position
    func peek(_ samples: UnsafeMutablePointer<Float>, count: Int) -> Int {
        let available = availableToRead
        let toPeek = min(count, available)

        if toPeek == 0 { return 0 }

        let read = sotf_atomic_load_u64(&readPosition)
        let readIndex = Int(read % UInt64(capacity))
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
        let read = sotf_atomic_load_u64(&readPosition)
        sotf_atomic_store_u64(&readPosition, read + UInt64(toSkip))
    }
}

/// Multi-channel audio ring buffer
final class MultiChannelRingBuffer {
    private let buffer: UnsafeMutablePointer<Float>
    private let framesCapacity: Int
    private var writePosition: UInt64 = 0
    private var readPosition: UInt64 = 0
    let channelCount: Int

    init(channelCount: Int, framesCapacity: Int) {
        self.channelCount = channelCount
        self.framesCapacity = framesCapacity
        self.buffer = UnsafeMutablePointer<Float>.allocate(
            capacity: framesCapacity * channelCount
        )
        self.buffer.initialize(repeating: 0, count: framesCapacity * channelCount)
    }

    deinit {
        buffer.deallocate()
    }

    func reset() {
        sotf_atomic_store_u64(&writePosition, 0)
        sotf_atomic_store_u64(&readPosition, 0)
        buffer.initialize(repeating: 0, count: framesCapacity * channelCount)
    }

    var availableFramesToRead: Int {
        let write = sotf_atomic_load_u64(&writePosition)
        let read = sotf_atomic_load_u64(&readPosition)
        guard write >= read else { return 0 }
        return Int(min(UInt64(framesCapacity), write - read))
    }

    var availableFramesToWrite: Int {
        framesCapacity - availableFramesToRead
    }

    /// Write interleaved audio data
    ///
    /// - Parameters:
    ///   - samples: Pointer to interleaved audio samples
    ///   - frameCount: Number of frames to write
    /// - Returns: Number of frames actually written
    ///
    /// - Note: The input buffer must contain at least `frameCount * channelCount` samples
    func writeInterleaved(_ samples: UnsafePointer<Float>, frameCount: Int) -> Int {
        guard frameCount > 0 && channelCount > 0 else { return 0 }

        let available = availableFramesToWrite
        let toWrite = min(frameCount, available)

        if toWrite == 0 { return 0 }

        let write = sotf_atomic_load_u64(&writePosition)
        let writeIndex = Int(write % UInt64(framesCapacity))
        let firstFrames = min(toWrite, framesCapacity - writeIndex)
        let secondFrames = toWrite - firstFrames

        memcpy(
            buffer.advanced(by: writeIndex * channelCount),
            samples,
            firstFrames * channelCount * MemoryLayout<Float>.size
        )
        if secondFrames > 0 {
            memcpy(
                buffer,
                samples.advanced(by: firstFrames * channelCount),
                secondFrames * channelCount * MemoryLayout<Float>.size
            )
        }

        // Publish every channel in the block with one release store.
        sotf_atomic_store_u64(&writePosition, write + UInt64(toWrite))
        return toWrite
    }

    /// Read to interleaved audio data
    ///
    /// - Parameters:
    ///   - samples: Pointer to output buffer for interleaved samples
    ///   - frameCount: Number of frames to read
    /// - Returns: Number of frames actually read
    ///
    /// - Note: The output buffer must have space for at least `frameCount * channelCount` samples
    func readInterleaved(_ samples: UnsafeMutablePointer<Float>, frameCount: Int) -> Int {
        guard frameCount > 0 && channelCount > 0 else { return 0 }

        let available = availableFramesToRead
        let toRead = min(frameCount, available)

        if toRead == 0 {
            memset(samples, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

        let read = sotf_atomic_load_u64(&readPosition)
        let readIndex = Int(read % UInt64(framesCapacity))
        let firstFrames = min(toRead, framesCapacity - readIndex)
        let secondFrames = toRead - firstFrames

        memcpy(
            samples,
            buffer.advanced(by: readIndex * channelCount),
            firstFrames * channelCount * MemoryLayout<Float>.size
        )
        if secondFrames > 0 {
            memcpy(
                samples.advanced(by: firstFrames * channelCount),
                buffer,
                secondFrames * channelCount * MemoryLayout<Float>.size
            )
        }

        sotf_atomic_store_u64(&readPosition, read + UInt64(toRead))

        // Fill remaining with silence
        if toRead < frameCount {
            let startIndex = toRead * channelCount
            memset(
                samples.advanced(by: startIndex),
                0,
                (frameCount * channelCount - startIndex) * MemoryLayout<Float>.size
            )
        }

        return toRead
    }

    /// Write non-interleaved (planar) audio data
    func writeNonInterleaved(_ buffers: [UnsafePointer<Float>], frameCount: Int) -> Int {
        guard buffers.count == channelCount else { return 0 }

        let available = availableFramesToWrite
        let toWrite = min(frameCount, available)

        if toWrite == 0 { return 0 }

        let write = sotf_atomic_load_u64(&writePosition)
        let writeIndex = Int(write % UInt64(framesCapacity))
        for frame in 0..<toWrite {
            let destinationFrame = (writeIndex + frame) % framesCapacity
            let destination = buffer.advanced(by: destinationFrame * channelCount)
            for channel in 0..<channelCount {
                destination[channel] = buffers[channel][frame]
            }
        }

        sotf_atomic_store_u64(&writePosition, write + UInt64(toWrite))
        return toWrite
    }

    /// Read to non-interleaved (planar) audio data
    func readNonInterleaved(_ buffers: [UnsafeMutablePointer<Float>], frameCount: Int) -> Int {
        guard buffers.count == channelCount else { return 0 }

        let available = availableFramesToRead
        let toRead = min(frameCount, available)

        if toRead > 0 {
            let read = sotf_atomic_load_u64(&readPosition)
            let readIndex = Int(read % UInt64(framesCapacity))
            for frame in 0..<toRead {
                let sourceFrame = (readIndex + frame) % framesCapacity
                let source = buffer.advanced(by: sourceFrame * channelCount)
                for channel in 0..<channelCount {
                    buffers[channel][frame] = source[channel]
                }
            }
            sotf_atomic_store_u64(&readPosition, read + UInt64(toRead))
        }

        for channel in 0..<channelCount {
            // Fill remaining with silence
            if toRead < frameCount {
                memset(
                    buffers[channel].advanced(by: toRead),
                    0,
                    (frameCount - toRead) * MemoryLayout<Float>.size
                )
            }
        }

        return toRead
    }
}
