// SharedMemory.swift - Shared memory interface for communication with Rust audio engine
//
// Security model:
// - Each user has their own shared memory region
// - Path is based on the console user's UID
// - Permissions allow only the user and _coreaudiod to access
// - The HAL driver (running as _coreaudiod) gets the console user's UID
//   to determine which shared memory region to use

import Darwin
import Foundation
import SystemConfiguration

/// Magic number for shared memory header validation: 'SOTF'
private let kSharedMemoryMagic: UInt32 = 0x534F5446

/// Current protocol version
/// Version 2: Added encryption fields (encrypted, key_fingerprint, frame_counter)
/// Version 3: Added config negotiation fields for bidirectional HAL-Daemon sync
/// Version 4: Added daemon heartbeat for stale-engine detection in the HAL driver
/// Version 5: Added configuring handshake for daemon-side reconfiguration
private let kSharedMemoryVersion: UInt32 = 5
private let kDaemonHeartbeatTimeoutMs: UInt64 = 3_000

/// Encrypted audio record magic: 'SEA1' (SotF Encrypted Audio v1)
private let kEncryptedRecordMagic: UInt32 = 0x5345_4131
private let kEncryptedRecordHeaderBytes = 24
private let kEncryptedRecordHeaderFloats = kEncryptedRecordHeaderBytes / 4

private func currentUnixMillis() -> UInt64 {
    return clock_gettime_nsec_np(CLOCK_REALTIME) / 1_000_000
}

/// Get the shared memory path for the current console user
///
/// Security model: each user has their own shared memory region.
/// Path is based on the console user's UID.
///
/// IMPORTANT: This must match the Rust side in shared_memory.rs which uses:
/// `/tmp/sotf-{uid}/audio.shm`
private func getSharedMemoryPath() -> String {
    let environment = ProcessInfo.processInfo.environment
    if let overridePath = environment["SOTF_HAL_SHARED_MEMORY_PATH"], !overridePath.isEmpty {
        return overridePath
    }
    if let runtimeDir = environment["SOTF_SYSTEMWIDE_RUNTIME_DIR"], !runtimeDir.isEmpty {
        return (runtimeDir as NSString).appendingPathComponent("audio.shm")
    }

    // Get the console user (the human logged in, not _coreaudiod)
    var uid: uid_t = 0
    var gid: gid_t = 0

    if SCDynamicStoreCopyConsoleUser(nil, &uid, &gid) != nil {
        let filePath = "/tmp/sotf-\(uid)/audio.shm"
        return filePath
    }

    // No console user - this shouldn't happen in normal operation
    halLog("ERROR: No console user found, cannot determine shared memory path")
    // Return a path that will fail to open, forcing proper error handling
    return "/tmp/sotf-unknown/audio.shm"
}

/// Header structure for shared memory region
/// Must match the Rust side exactly (SharedAudioHeader in shared_memory.rs)
struct SharedAudioHeader {
    var magic: UInt32           // 0x534F5446 ('SOTF')
    var version: UInt32         // Protocol version
    var sampleRate: UInt32      // Current sample rate
    var bufferFrames: UInt32    // Frames per buffer
    var channelCount: UInt32    // Number of channels

    // Ring buffer state (atomic on both sides)
    var writePosition: UInt64   // Write position in samples
    var readPosition: UInt64    // Read position in samples

    // Control flags (atomic)
    var active: UInt32          // IO is running
    var configChanged: UInt32   // Config change notification
    var driverReady: UInt32     // Driver is initialized
    var engineReady: UInt32     // Rust engine is connected

    // Encryption fields (version 2+)
    var encrypted: UInt32       // 0 = disabled, 1 = enabled
    // First 8 bytes of SHA256, stored as a UInt64 with canonical big-endian
    // conversion. This mirrors Rust's AtomicU64 field and keeps the offset
    // aligned at byte 64.
    var keyFingerprint: UInt64
    var frameCounter: UInt64    // Frame counter for nonce generation

    // Config negotiation fields (version 3+)
    var requestedSampleRate: UInt32     // Requested sample rate
    var requestedBufferFrames: UInt32   // Requested buffer frames
    var actualSampleRate: UInt32        // Actual sample rate in use
    var actualBufferFrames: UInt32      // Actual buffer frames in use
    var configStatus: UInt32            // 0=pending, 1=accepted, 2=negotiated, 3=error
    var configSource: UInt32            // 1=HAL initiated, 2=Daemon initiated
    var configErrorCode: UInt32         // Error code if configStatus=3

    // Statistics
    var encryptionOverflowCount: UInt64 // Encrypted write drops due to insufficient ring space
    var daemonHeartbeatMs: UInt64       // Daemon liveness heartbeat in Unix epoch milliseconds

    // Reconfiguration handshake (version 5+)
    var configuring: UInt32             // Daemon is mutating geometry/ring positions
}

private struct EncryptedRecordMetadata {
    let sampleCount: Int
    let frameCounter: UInt64
    let ciphertextLen: Int
    let totalBytes: Int
    let floatCount: Int
}

/// Shared memory buffer for audio exchange with Rust engine
final class SharedAudioBuffer {
    private var sharedMemory: UnsafeMutableRawPointer?
    private var memorySize: Int = 0
    private var fileDescriptor: Int32 = -1

    /// The path to the shared memory file (stored for cleanup)
    private var currentPath: String = ""

    /// Number of audio buffers in the ring (power of 2 recommended)
    private let numBuffers = 8

    /// Header pointer (start of shared memory)
    private var header: UnsafeMutablePointer<SharedAudioHeader>? {
        sharedMemory?.assumingMemoryBound(to: SharedAudioHeader.self)
    }

    /// Audio data pointer (after header)
    private var audioData: UnsafeMutablePointer<Float>? {
        guard let mem = sharedMemory else { return nil }
        let headerSize = MemoryLayout<SharedAudioHeader>.size
        // Align to 64 bytes for SIMD
        let alignedOffset = (headerSize + 63) & ~63
        return mem.advanced(by: alignedOffset).assumingMemoryBound(to: Float.self)
    }

    /// Total capacity in samples
    private var audioCapacity: Int = 0
    private var encryptedPlaintextScratch: [UInt8] = []
    private var encryptedPayloadScratch: [UInt8] = []
    private var encryptedHeaderScratch: [UInt8] = []

    /// Whether we're connected to shared memory
    var isConnected: Bool {
        let hasMem = sharedMemory != nil
        let hasMagic = header?.pointee.magic == kSharedMemoryMagic
        return hasMem && hasMagic
    }

    /// Whether the Rust engine is connected.
    ///
    /// Important: when `header` is nil (shared memory never initialised or
    /// init failed), we must return `false`. The previous formulation
    /// `header?.pointee.engineReady != 0` returns `true` when `header` is nil
    /// because Swift treats `nil != 0` as true for optional comparisons —
    /// callers then see `engineReady=1, isConnected=0` which is nonsense.
    var engineReady: Bool {
        guard let header = header, header.pointee.engineReady != 0 else {
            return false
        }

        // Protocol v4 treats engineReady as valid only while the daemon keeps
        // refreshing its heartbeat. This prevents CoreAudio from filling the
        // ring forever after a daemon crash/stall leaves engineReady stuck at 1.
        if header.pointee.version < 4 {
            return true
        }

        let heartbeat = header.pointee.daemonHeartbeatMs
        guard heartbeat != 0 else {
            return false
        }

        let now = currentUnixMillis()
        if now < heartbeat {
            return true
        }
        return now - heartbeat <= kDaemonHeartbeatTimeoutMs
    }

    /// Debug: get connection state details
    var connectionStateDebug: String {
        let hasMem = sharedMemory != nil
        let magic = header?.pointee.magic ?? 0
        let expectedMagic = kSharedMemoryMagic
        let magicMatch = magic == expectedMagic
        let version = header?.pointee.version ?? 0
        let engineFlag = header?.pointee.engineReady ?? 0
        let heartbeat = (version >= 4) ? (header?.pointee.daemonHeartbeatMs ?? 0) : 0
        let heartbeatAge: String
        if heartbeat == 0 {
            heartbeatAge = "none"
        } else {
            let now = currentUnixMillis()
            heartbeatAge = now >= heartbeat ? "\(now - heartbeat)ms" : "future"
        }
        return "mem=\(hasMem), magic=0x\(String(format: "%08X", magic)) (expected 0x\(String(format: "%08X", expectedMagic))), magicMatch=\(magicMatch), version=\(version), engineReadyFlag=\(engineFlag), engineReady=\(engineReady), daemonHeartbeatMs=\(heartbeat), daemonHeartbeatAge=\(heartbeatAge)"
    }

    /// Initialize shared memory
    /// - Parameters:
    ///   - sampleRate: Sample rate in Hz
    ///   - bufferFrames: Frames per buffer
    ///   - channelCount: Number of audio channels
    /// - Returns: true if successful
    func initialize(sampleRate: UInt32, bufferFrames: UInt32, channelCount: UInt32) -> Bool {
        // Calculate sizes
        let headerSize = MemoryLayout<SharedAudioHeader>.size
        let alignedHeaderSize = (headerSize + 63) & ~63  // 64-byte aligned
        audioCapacity = Int(bufferFrames) * Int(channelCount) * numBuffers
        let audioSize = audioCapacity * MemoryLayout<Float>.size
        let requiredMemorySize = alignedHeaderSize + audioSize

        // Get the shared memory path (secure per-user path or legacy)
        currentPath = getSharedMemoryPath()
        halLog("SharedMemory: initializing \(requiredMemorySize) bytes at \(currentPath)")

        if sharedMemory != nil || fileDescriptor >= 0 {
            closeSharedMemory()
        }

        // Open the daemon-owned file. The HAL plugin runs inside coreaudiod's
        // restricted environment, so creation/sizing/permissions are handled by
        // the daemon before IO starts.
        fileDescriptor = Darwin.open(currentPath, O_RDWR)
        if fileDescriptor < 0 {
            halLog("SharedMemory: open failed: \(String(cString: strerror(errno)))")
            return false
        }

        var statBuf = stat()
        if fstat(fileDescriptor, &statBuf) != 0 {
            halLog("SharedMemory: fstat failed: \(String(cString: strerror(errno)))")
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
            return false
        }

        if statBuf.st_size < requiredMemorySize {
            halLog("SharedMemory: file too small: \(statBuf.st_size), need \(requiredMemorySize)")
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
            return false
        }
        memorySize = Int(statBuf.st_size)
        let mappedAudioBytes = max(audioSize, max(0, memorySize - alignedHeaderSize))
        encryptedPlaintextScratch = [UInt8](repeating: 0, count: mappedAudioBytes)
        encryptedPayloadScratch = [UInt8](repeating: 0, count: mappedAudioBytes)
        encryptedHeaderScratch = [UInt8](repeating: 0, count: kEncryptedRecordHeaderBytes)

        // Map memory
        sharedMemory = mmap(nil, memorySize, PROT_READ | PROT_WRITE, MAP_SHARED, fileDescriptor, 0)
        if sharedMemory == MAP_FAILED {
            halLog("SharedMemory: mmap failed: \(String(cString: strerror(errno)))")
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
            sharedMemory = nil
            return false
        }

        // Initialize header
        guard let header = header else { return false }

        // If shared memory was already initialized by the daemon (running before
        // coreaudiod was respawned), preserve its runtime state — engineReady,
        // ring positions, encryption session, config-negotiation state. Wiping
        // those silently flips engineReady from 1 → 0 behind the daemon's back
        // and stops the audio path. Only fresh memory should be zeroed.
        let alreadyInitialized = (header.pointee.magic == kSharedMemoryMagic)

        if !alreadyInitialized {
            halLog("SharedMemory: existing file is not daemon-initialized")
            munmap(sharedMemory!, memorySize)
            sharedMemory = nil
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
            return false
        }

        halLog("SharedMemory: already initialized, preserving daemon state")

        // Always set: identifying fields and structural geometry.
        // Geometry can legitimately change across coreaudiod re-spawns even
        // when the daemon was running (different sample rate, different buffer
        // size). The daemon tolerates this through its config-negotiation path.
        header.pointee.magic = kSharedMemoryMagic
        header.pointee.version = kSharedMemoryVersion
        header.pointee.sampleRate = sampleRate
        header.pointee.bufferFrames = bufferFrames
        header.pointee.channelCount = channelCount
        header.pointee.actualSampleRate = sampleRate
        header.pointee.actualBufferFrames = bufferFrames
        header.pointee.driverReady = 1

        OSMemoryBarrier()

        halLog("SharedMemory: initialized (version \(kSharedMemoryVersion), fresh=\(!alreadyInitialized), engineReady=\(header.pointee.engineReady))")
        return true
    }

    /// Close shared memory
    func closeSharedMemory() {
        if let header = header {
            header.pointee.driverReady = 0
            OSMemoryBarrier()
        }

        if let mem = sharedMemory, memorySize > 0 {
            munmap(mem, memorySize)
        }
        sharedMemory = nil

        if fileDescriptor >= 0 {
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
        }

        halLog("SharedMemory: closed")
    }

    /// Set active state
    func setActive(_ active: Bool) {
        guard let header = header else { return }
        header.pointee.active = active ? 1 : 0
        OSMemoryBarrier()
    }

    /// Signal configuration change to Rust engine
    func signalConfigChange() {
        guard let header = header else { return }
        header.pointee.configChanged = 1
        OSMemoryBarrier()
    }

    /// Update sample rate (called when CoreAudio changes the device sample rate)
    /// This uses the config negotiation protocol to notify the daemon
    func updateSampleRate(_ sampleRate: UInt32) {
        guard let header = header else { return }
        header.pointee.sampleRate = sampleRate
        // Use config negotiation so daemon knows to reconfigure
        header.pointee.requestedSampleRate = sampleRate
        header.pointee.requestedBufferFrames = header.pointee.bufferFrames
        header.pointee.configStatus = 0  // pending
        header.pointee.configSource = 1  // HAL initiated
        OSMemoryBarrier()
        header.pointee.configChanged = 1
        halLog("updateSampleRate: requested \(sampleRate)Hz via config negotiation")
    }

    private func fingerprintMatches(_ headerFingerprint: UInt64, _ cipherFingerprint: [UInt8]) -> Bool {
        guard cipherFingerprint.count == 8 else { return false }

        for index in 0..<8 {
            let shift = UInt64((7 - index) * 8)
            let byte = UInt8((headerFingerprint >> shift) & 0xff)
            if byte != cipherFingerprint[index] {
                return false
            }
        }
        return true
    }

    private func cipherMatchingHeader(_ header: UnsafeMutablePointer<SharedAudioHeader>) -> AudioCipher? {
        OSMemoryBarrier()
        let headerFingerprint = header.pointee.keyFingerprint

        if let cipher = EncryptionKeyManager.shared.getCipher(),
           fingerprintMatches(headerFingerprint, cipher.getFingerprint()) {
            return cipher
        }

        _ = EncryptionKeyManager.shared.checkAndReload()

        if let cipher = EncryptionKeyManager.shared.getCipher(),
           fingerprintMatches(headerFingerprint, cipher.getFingerprint()) {
            return cipher
        }

        return nil
    }

    private func writeUInt32BE(_ value: UInt32, into bytes: inout [UInt8], at offset: Int) {
        bytes[offset] = UInt8((value >> 24) & 0xff)
        bytes[offset + 1] = UInt8((value >> 16) & 0xff)
        bytes[offset + 2] = UInt8((value >> 8) & 0xff)
        bytes[offset + 3] = UInt8(value & 0xff)
    }

    private func writeUInt64BE(_ value: UInt64, into bytes: inout [UInt8], at offset: Int) {
        for index in 0..<8 {
            let shift = UInt64((7 - index) * 8)
            bytes[offset + index] = UInt8((value >> shift) & 0xff)
        }
    }

    private func readUInt32BE(_ bytes: [UInt8], at offset: Int) -> UInt32 {
        return (UInt32(bytes[offset]) << 24) |
               (UInt32(bytes[offset + 1]) << 16) |
               (UInt32(bytes[offset + 2]) << 8) |
               UInt32(bytes[offset + 3])
    }

    private func readUInt64BE(_ bytes: [UInt8], at offset: Int) -> UInt64 {
        var value: UInt64 = 0
        for index in 0..<8 {
            value = (value << 8) | UInt64(bytes[offset + index])
        }
        return value
    }

    private func writeEncryptedRecordHeader(_ bytes: inout [UInt8], sampleCount: Int, frameCounter: UInt64, ciphertextLen: Int) -> Bool {
        guard bytes.count >= kEncryptedRecordHeaderBytes,
              sampleCount > 0,
              sampleCount <= Int(UInt32.max),
              ciphertextLen <= Int(UInt32.max) else {
            return false
        }

        writeUInt32BE(kEncryptedRecordMagic, into: &bytes, at: 0)
        writeUInt32BE(UInt32(sampleCount), into: &bytes, at: 4)
        writeUInt64BE(frameCounter, into: &bytes, at: 8)
        writeUInt32BE(UInt32(ciphertextLen), into: &bytes, at: 16)
        writeUInt32BE(0, into: &bytes, at: 20)
        return true
    }

    private func parseEncryptedRecordHeader(_ bytes: [UInt8]) -> EncryptedRecordMetadata? {
        guard bytes.count >= kEncryptedRecordHeaderBytes else { return nil }

        let magic = readUInt32BE(bytes, at: 0)
        guard magic == kEncryptedRecordMagic else { return nil }

        let sampleCount = Int(readUInt32BE(bytes, at: 4))
        let frameCounter = readUInt64BE(bytes, at: 8)
        let ciphertextLen = Int(readUInt32BE(bytes, at: 16))
        let reserved = readUInt32BE(bytes, at: 20)
        let expectedCiphertextLen = sampleCount * MemoryLayout<Float>.size + 16

        guard sampleCount > 0,
              reserved == 0,
              ciphertextLen == expectedCiphertextLen else {
            return nil
        }

        let totalBytes = kEncryptedRecordHeaderBytes + ciphertextLen
        return EncryptedRecordMetadata(
            sampleCount: sampleCount,
            frameCounter: frameCounter,
            ciphertextLen: ciphertextLen,
            totalBytes: totalBytes,
            floatCount: (totalBytes + 3) / 4
        )
    }

    private func clampedUsedSampleCount(writePos: UInt64, readPos: UInt64, capacity: Int) -> Int {
        guard capacity > 0, writePos >= readPos else { return 0 }

        let distance = writePos - readPos
        let capacity64 = UInt64(capacity)
        if distance >= capacity64 {
            return capacity
        }
        return Int(distance)
    }

    /// Write audio to shared memory (called from DoIOOperation for output)
    /// Uses lock-free ring buffer algorithm
    func writeAudio(_ buffer: UnsafePointer<Float>, frameCount: Int, channelCount: Int) -> Int {
        guard let header = header, let audioData = audioData else { return 0 }
        guard frameCount > 0, channelCount > 0, audioCapacity > 0 else { return 0 }
        if header.pointee.configuring != 0 {
            return 0
        }

        // Check for encryption
        if header.pointee.encrypted != 0 {
            if let cipher = cipherMatchingHeader(header) {
                let sampleCount = frameCount * channelCount
                let ciphertextLen = sampleCount * MemoryLayout<Float>.size + 16
                let totalBytes = kEncryptedRecordHeaderBytes + ciphertextLen
                guard totalBytes <= encryptedPayloadScratch.count else {
                    header.pointee.encryptionOverflowCount += 1
                    return 0
                }

                let frameCounter = sotf_atomic_fetch_add_u64(&header.pointee.frameCounter, 1)
                guard writeEncryptedRecordHeader(
                    &encryptedPayloadScratch,
                    sampleCount: sampleCount,
                    frameCounter: frameCounter,
                    ciphertextLen: ciphertextLen
                ) else {
                    return 0
                }

                let bytesWritten = encryptedPayloadScratch.withUnsafeMutableBytes { rawBuffer -> Int in
                    guard let baseAddress = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                        return 0
                    }
                    return cipher.encryptToBuffer(
                        buffer,
                        sampleCount: sampleCount,
                        frameCounter: frameCounter,
                        output: baseAddress.advanced(by: kEncryptedRecordHeaderBytes),
                        plaintextScratch: &encryptedPlaintextScratch
                    )
                }

                if bytesWritten == ciphertextLen {
                    return writeRawBytes(
                        encryptedPayloadScratch,
                        byteCount: totalBytes,
                        originalFrameCount: frameCount
                    )
                }
            }
            // If encryption enabled but failed, write silence or return 0
            return 0
        }

        // Standard unencrypted write
        let sampleCount = frameCount * channelCount
        let writableCapacity = audioCapacity - (audioCapacity % channelCount)
        guard sampleCount > 0, writableCapacity > 0 else { return 0 }

        let samplesToWrite = min(sampleCount, writableCapacity)
        let sourceOffset = sampleCount - samplesToWrite
        let sourceBuffer = buffer.advanced(by: sourceOffset)
        let writePos = header.pointee.writePosition
        let readPos = header.pointee.readPosition

        // This is a live capture stream. If the daemon falls behind during a
        // reconnect/client switch, drop stale samples and always publish the
        // newest complete frames instead of returning 0 forever on a full ring.
        let used = clampedUsedSampleCount(
            writePos: writePos,
            readPos: readPos,
            capacity: writableCapacity
        )
        let effectiveReadPos = writePos >= readPos ? readPos : writePos
        let samplesToDrop = max(0, used + samplesToWrite - writableCapacity)
        let adjustedReadPos = effectiveReadPos + UInt64(samplesToDrop)

        if adjustedReadPos != readPos {
            OSMemoryBarrier()
            header.pointee.readPosition = adjustedReadPos
        }

        let writeIndex = Int(writePos % UInt64(audioCapacity))
        let firstPart = min(samplesToWrite, audioCapacity - writeIndex)
        let secondPart = samplesToWrite - firstPart

        // Copy data
        memcpy(audioData.advanced(by: writeIndex), sourceBuffer, firstPart * MemoryLayout<Float>.size)
        if secondPart > 0 {
            memcpy(audioData, sourceBuffer.advanced(by: firstPart), secondPart * MemoryLayout<Float>.size)
        }

        // Update write position atomically
        OSMemoryBarrier()
        if header.pointee.active == 0 {
            header.pointee.active = 1
        }
        header.pointee.writePosition = writePos + UInt64(samplesToWrite)

        // TRACE: Log successful write to shared memory ring buffer
        // Note: Avoid logging every frame in production (use os_signpost for performance tracing)
        #if DEBUG
        if samplesToWrite > 0 {
            let newWritePos = writePos + UInt64(samplesToWrite)
            halLog("[SHM TRACE] write: \(samplesToWrite/channelCount) frames, dropped=\(samplesToDrop/channelCount) frames, wpos=\(newWritePos), rpos=\(adjustedReadPos)")
        }
        #endif

        return samplesToWrite / channelCount
    }

    /// Helper to write raw bytes (ciphertext) to ring buffer
    private func writeRawBytes(_ bytes: [UInt8], byteCount: Int, originalFrameCount: Int) -> Int {
        guard let header = header, let audioData = audioData else { return 0 }
        if header.pointee.configuring != 0 {
            return 0
        }
        
        // Calculate required space in floats (rounded up)
        let floatCount = (byteCount + 3) / 4
        guard floatCount > 0, audioCapacity > 0 else { return 0 }

        if floatCount > audioCapacity {
            header.pointee.encryptionOverflowCount += 1
            return 0
        }
        
        let writePos = header.pointee.writePosition
        let readPos = header.pointee.readPosition
        let used = clampedUsedSampleCount(
            writePos: writePos,
            readPos: readPos,
            capacity: audioCapacity
        )
        let available = audioCapacity - used
        
        if floatCount > available {
            header.pointee.encryptionOverflowCount += 1
            OSMemoryBarrier()
            header.pointee.readPosition = writePos
        } else if writePos < readPos {
            OSMemoryBarrier()
            header.pointee.readPosition = writePos
        }
        
        let writeIndex = Int(writePos % UInt64(audioCapacity))
        let firstPartFloats = min(floatCount, audioCapacity - writeIndex)
        let firstPartBytes = firstPartFloats * 4
        
        let bytesToWriteFirst = min(byteCount, firstPartBytes)
        let bytesToWriteSecond = byteCount - bytesToWriteFirst
        
        bytes.withUnsafeBufferPointer { ptr in
            guard let base = ptr.baseAddress else { return }
            
            // First part
            let destFirst = UnsafeMutableRawPointer(audioData.advanced(by: writeIndex))
            memcpy(destFirst, base, bytesToWriteFirst)
            
            // Second part (wrap)
            if bytesToWriteSecond > 0 {
                let destSecond = UnsafeMutableRawPointer(audioData) // Start of buffer
                memcpy(destSecond, base.advanced(by: bytesToWriteFirst), bytesToWriteSecond)
            }
        }
        
        OSMemoryBarrier()
        if header.pointee.active == 0 {
            header.pointee.active = 1
        }
        header.pointee.writePosition = writePos + UInt64(floatCount)
        
        return originalFrameCount
    }

    /// Read audio from shared memory (called from DoIOOperation for input)
    /// Uses lock-free ring buffer algorithm
    func readAudio(_ buffer: UnsafeMutablePointer<Float>, frameCount: Int, channelCount: Int) -> Int {
        guard let header = header, let audioData = audioData else {
            // Fill with silence
            memset(buffer, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

        // Check for encryption
        if header.pointee.encrypted != 0 {
            if let cipher = cipherMatchingHeader(header) {
                let requestedSampleCount = frameCount * channelCount
                let writePos = header.pointee.writePosition
                let readPos = header.pointee.readPosition
                let available = Int(writePos - readPos)

                if available >= kEncryptedRecordHeaderFloats {
                    copyRawBytes(
                        at: readPos,
                        into: &encryptedHeaderScratch,
                        byteCount: kEncryptedRecordHeaderBytes,
                        floatCount: kEncryptedRecordHeaderFloats
                    )

                    guard let record = parseEncryptedRecordHeader(encryptedHeaderScratch) else {
                        OSMemoryBarrier()
                        header.pointee.readPosition = writePos
                        memset(buffer, 0, requestedSampleCount * MemoryLayout<Float>.size)
                        return 0
                    }

                    if record.floatCount <= available && record.sampleCount <= requestedSampleCount {
                        guard record.totalBytes <= encryptedPayloadScratch.count else {
                            OSMemoryBarrier()
                            header.pointee.readPosition = readPos + UInt64(record.floatCount)
                            memset(buffer, 0, requestedSampleCount * MemoryLayout<Float>.size)
                            return 0
                        }
                        copyRawBytes(
                            at: readPos,
                            into: &encryptedPayloadScratch,
                            byteCount: record.totalBytes,
                            floatCount: record.floatCount
                        )

                        let decryptedCount = encryptedPayloadScratch.withUnsafeBufferPointer { ptr in
                            guard let base = ptr.baseAddress else { return 0 }
                            return cipher.decryptFromBuffer(
                                base.advanced(by: kEncryptedRecordHeaderBytes),
                                ciphertextLen: record.ciphertextLen,
                                frameCounter: record.frameCounter,
                                output: buffer
                            )
                        }

                        OSMemoryBarrier()
                        header.pointee.readPosition = readPos + UInt64(record.floatCount)

                        if decryptedCount > 0 {
                            if decryptedCount < requestedSampleCount {
                                memset(
                                    buffer.advanced(by: decryptedCount),
                                    0,
                                    (requestedSampleCount - decryptedCount) * MemoryLayout<Float>.size
                                )
                            }
                            return decryptedCount / channelCount
                        }
                    }
                }
            }
            // Decryption failed or no key
            memset(buffer, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

        // Standard unencrypted read
        let sampleCount = frameCount * channelCount
        let writePos = header.pointee.writePosition
        let readPos = header.pointee.readPosition

        // Calculate available data
        let available = Int(writePos - readPos)
        let toRead = min(sampleCount, available)

        if toRead <= 0 {
            // Fill with silence
            memset(buffer, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

        let readIndex = Int(readPos % UInt64(audioCapacity))
        let firstPart = min(toRead, audioCapacity - readIndex)
        let secondPart = toRead - firstPart

        // Copy data
        memcpy(buffer, audioData.advanced(by: readIndex), firstPart * MemoryLayout<Float>.size)
        if secondPart > 0 {
            memcpy(buffer.advanced(by: firstPart), audioData, secondPart * MemoryLayout<Float>.size)
        }

        // Fill remaining with silence
        if toRead < sampleCount {
            memset(buffer.advanced(by: toRead), 0, (sampleCount - toRead) * MemoryLayout<Float>.size)
        }

        // Update read position atomically
        OSMemoryBarrier()
        header.pointee.readPosition = readPos + UInt64(toRead)

        // TRACE: Log successful read from shared memory ring buffer
        #if DEBUG
        if toRead > 0 {
            let newReadPos = readPos + UInt64(toRead)
            halLog("[SHM TRACE] read: \(toRead/channelCount) frames, wpos=\(writePos), rpos=\(newReadPos)")
        }
        #endif

        return toRead / channelCount
    }

    /// Helper to read raw bytes (ciphertext) from ring buffer
    private func copyRawBytes(at position: UInt64, into buffer: inout [UInt8], byteCount: Int, floatCount: Int) {
        guard let audioData = audioData else { return }
        guard byteCount <= buffer.count else { return }

        let readIndex = Int(position % UInt64(audioCapacity))
        let firstPartFloats = min(floatCount, audioCapacity - readIndex)
        let firstPartBytes = firstPartFloats * MemoryLayout<Float>.size

        let bytesToReadFirst = min(byteCount, firstPartBytes)
        let bytesToReadSecond = byteCount - bytesToReadFirst

        buffer.withUnsafeMutableBufferPointer { ptr in
            guard let base = ptr.baseAddress else { return }

            let srcFirst = UnsafeRawPointer(audioData.advanced(by: readIndex))
            memcpy(base, srcFirst, bytesToReadFirst)

            if bytesToReadSecond > 0 {
                let srcSecond = UnsafeRawPointer(audioData)
                memcpy(base.advanced(by: bytesToReadFirst), srcSecond, bytesToReadSecond)
            }
        }
    }

    private func readRawBytes(_ buffer: inout [UInt8], floatCount: Int) {
        guard let header = header else { return }
        
        let readPos = header.pointee.readPosition
        copyRawBytes(at: readPos, into: &buffer, byteCount: buffer.count, floatCount: floatCount)

        OSMemoryBarrier()
        header.pointee.readPosition = readPos + UInt64(floatCount)
    }

    // MARK: - Config Negotiation Methods (version 3+)

    /// Check if configuration change is pending
    func configChanged() -> Bool {
        OSMemoryBarrier()
        return header?.pointee.configChanged != 0
    }

    /// Get config source (1=HAL initiated, 2=Daemon initiated)
    func configSource() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.configSource ?? 0
    }

    /// Get actual sample rate (set by responder after negotiation)
    ///
    /// Caller should check `getConfigStatus()` first, which performs a memory barrier.
    func getActualSampleRate() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.actualSampleRate ?? 0
    }

    /// Get actual buffer frames (set by responder after negotiation)
    ///
    /// Caller should check `getConfigStatus()` first, which performs a memory barrier.
    func getActualBufferFrames() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.actualBufferFrames ?? 0
    }

    /// Get config status (0=pending, 1=accepted, 2=negotiated, 3=error)
    ///
    /// This function includes a memory barrier to ensure visibility of
    /// the status and all related config values from the responder.
    func getConfigStatus() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.configStatus ?? 0
    }

    /// Get config error code (only valid when configStatus=3)
    ///
    /// Caller should check `getConfigStatus()` first, which performs a memory barrier.
    func getConfigErrorCode() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.configErrorCode ?? 0
    }

    /// Get requested sample rate (set by the config requester)
    func getRequestedSampleRate() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.requestedSampleRate ?? 0
    }

    /// Get requested buffer frames (set by the config requester)
    func getRequestedBufferFrames() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.requestedBufferFrames ?? 0
    }

    /// Get requested/current channel count.
    ///
    /// Protocol v4 does not have a separate requested-channel field; daemon
    /// initiated channel changes publish the desired count in `channelCount`
    /// before setting `configChanged`.
    func getRequestedChannelCount() -> UInt32 {
        OSMemoryBarrier()
        return header?.pointee.channelCount ?? 0
    }

    /// Request a configuration change (called by HAL when client changes sample rate)
    /// Sets configSource=1 (HAL initiated) and configChanged=1
    ///
    /// Memory ordering: All non-atomic fields are written first, then a memory
    /// barrier ensures they are visible before setting configChanged. The
    /// configChanged flag acts as the notification point for the responder.
    func requestConfigChange(sampleRate: UInt32, bufferFrames: UInt32, channelCount: UInt32) {
        guard let header = header else { return }
        if header.pointee.configuring != 0 {
            return
        }
        header.pointee.requestedSampleRate = sampleRate
        header.pointee.requestedBufferFrames = bufferFrames
        header.pointee.channelCount = channelCount
        header.pointee.configStatus = 0  // pending
        header.pointee.configSource = 1  // HAL initiated
        // Memory barrier ensures non-atomic writes are visible before flag
        OSMemoryBarrier()
        header.pointee.configChanged = 1
        // Note: No trailing barrier needed - configChanged acts as the release point
    }

    /// Wait for daemon to acknowledge config change
    /// Returns true if accepted (status=1), false otherwise
    func waitForConfigAck(timeout: Int) -> Bool {
        let start = DispatchTime.now()
        while true {
            OSMemoryBarrier()
            let status = header?.pointee.configStatus ?? 0
            if status != 0 {  // Not pending anymore
                return status == 1  // 1 = accepted
            }
            if DispatchTime.now() > start + .milliseconds(timeout) {
                return false  // Timeout
            }
            usleep(10_000)  // 10ms
        }
    }

    /// Set config status (atomic)
    func setConfigStatus(_ status: UInt32) {
        guard let header = header else { return }
        header.pointee.configStatus = status
        OSMemoryBarrier()
    }

    /// Acknowledge a daemon-initiated config change.
    func acknowledgeConfigChange(actualSampleRate: UInt32, actualBufferFrames: UInt32, status: UInt32, errorCode: UInt32) {
        guard let header = header else { return }
        header.pointee.actualSampleRate = actualSampleRate
        header.pointee.actualBufferFrames = actualBufferFrames
        header.pointee.configErrorCode = errorCode
        OSMemoryBarrier()
        header.pointee.configStatus = status
        header.pointee.configChanged = 0
    }

    /// Clear config changed flag (called after handling daemon-initiated change)
    func clearConfigChanged() {
        guard let header = header else { return }
        header.pointee.configChanged = 0
        OSMemoryBarrier()
    }

    deinit {
        closeSharedMemory()
    }
}
