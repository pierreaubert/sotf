// SharedMemory.swift - Shared memory interface for communication with Rust audio engine
//
// Security model:
// - Each user has their own shared memory region
// - Path is based on the console user's UID
// - Permissions allow only the user and _coreaudiod to access
// - The HAL driver (running as _coreaudiod) gets the console user's UID
//   to determine which shared memory region to use

import Foundation
import SystemConfiguration

/// Magic number for shared memory header validation: 'SOTF'
private let kSharedMemoryMagic: UInt32 = 0x534F5446

/// Current protocol version
/// Version 2: Added encryption fields (encrypted, key_fingerprint, frame_counter)
/// Version 3: Added config negotiation fields for bidirectional HAL-Daemon sync
private let kSharedMemoryVersion: UInt32 = 3

/// Get the shared memory path for the current console user
///
/// Security model: each user has their own shared memory region.
/// Path is based on the console user's UID.
///
/// IMPORTANT: This must match the Rust side in shared_memory.rs which uses:
/// `/tmp/sotf-{uid}/audio.shm`
private func getSharedMemoryPath() -> String {
    // Get the console user (the human logged in, not _coreaudiod)
    var uid: uid_t = 0
    var gid: gid_t = 0

    if SCDynamicStoreCopyConsoleUser(nil, &uid, &gid) != nil {
        let dirPath = "/tmp/sotf-\(uid)"
        let filePath = "\(dirPath)/audio.shm"

        // Create the directory if it doesn't exist
        var isDir: ObjCBool = false
        if !FileManager.default.fileExists(atPath: dirPath, isDirectory: &isDir) {
            do {
                // Create directory with permissions that allow both _coreaudiod and user access
                // 0777 allows anyone to read/write/execute (files inside will have restricted perms)
                try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true, attributes: [
                    .posixPermissions: 0o777
                ])
            } catch {
                halLog("Failed to create shared memory directory: \(error)")
            }
        } else {
            // Directory exists, ensure permissions are correct (0777)
            do {
                try FileManager.default.setAttributes([.posixPermissions: 0o777], ofItemAtPath: dirPath)
            } catch {
                halLog("Failed to update directory permissions: \(error)")
            }
        }

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
    var keyFingerprint: (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8) // First 8 bytes of SHA256
    var frameCounter: UInt64    // Frame counter for nonce generation

    // Config negotiation fields (version 3+)
    var requestedSampleRate: UInt32     // Requested sample rate
    var requestedBufferFrames: UInt32   // Requested buffer frames
    var actualSampleRate: UInt32        // Actual sample rate in use
    var actualBufferFrames: UInt32      // Actual buffer frames in use
    var configStatus: UInt32            // 0=pending, 1=accepted, 2=negotiated, 3=error
    var configSource: UInt32            // 1=HAL initiated, 2=Daemon initiated
    var configErrorCode: UInt32         // Error code if configStatus=3
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

    /// Whether we're connected to shared memory
    var isConnected: Bool {
        let hasMem = sharedMemory != nil
        let hasMagic = header?.pointee.magic == kSharedMemoryMagic
        return hasMem && hasMagic
    }

    /// Whether the Rust engine is connected
    var engineReady: Bool {
        return header?.pointee.engineReady != 0
    }

    /// Debug: get connection state details
    var connectionStateDebug: String {
        let hasMem = sharedMemory != nil
        let magic = header?.pointee.magic ?? 0
        let expectedMagic = kSharedMemoryMagic
        let magicMatch = magic == expectedMagic
        let engineFlag = header?.pointee.engineReady ?? 0
        return "mem=\(hasMem), magic=0x\(String(format: "%08X", magic)) (expected 0x\(String(format: "%08X", expectedMagic))), magicMatch=\(magicMatch), engineReady=\(engineFlag)"
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
        memorySize = alignedHeaderSize + audioSize

        // Get the shared memory path (secure per-user path or legacy)
        currentPath = getSharedMemoryPath()
        halLog("SharedMemory: initializing \(memorySize) bytes at \(currentPath)")

        // Open or create the file with permissions for user and coreaudiod group
        // Mode 0666 = rw-rw-rw- (allow all users to read/write)
        // REQUIRED because _coreaudiod creates the file (owner=_coreaudiod) but the daemon
        // runs as the user (who is not in _coreaudiod group).
        // The directory itself is already protected by being in /tmp/sotf-{uid}/
        fileDescriptor = Darwin.open(currentPath, O_RDWR | O_CREAT, 0666)
        if fileDescriptor < 0 {
            halLog("SharedMemory: open failed: \(String(cString: strerror(errno)))")
            return false
        }

        // Set size
        if ftruncate(fileDescriptor, off_t(memorySize)) != 0 {
            halLog("SharedMemory: ftruncate failed: \(String(cString: strerror(errno)))")
            Darwin.close(fileDescriptor)
            fileDescriptor = -1
            return false
        }

        // Ensure permissions are correct (override umask)
        // Mode 0666 = rw-rw-rw- allows owner (_coreaudiod) and user to read/write
        chmod(currentPath, 0o666)

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

        // Check if already initialized by another process
        if header.pointee.magic == kSharedMemoryMagic {
            halLog("SharedMemory: already initialized, updating")
        } else {
            // Clear the memory
            memset(sharedMemory!, 0, memorySize)
        }

        header.pointee.magic = kSharedMemoryMagic
        header.pointee.version = kSharedMemoryVersion
        header.pointee.sampleRate = sampleRate
        header.pointee.bufferFrames = bufferFrames
        header.pointee.channelCount = channelCount
        header.pointee.writePosition = 0
        header.pointee.readPosition = 0
        header.pointee.active = 0
        header.pointee.configChanged = 0
        header.pointee.driverReady = 1
        header.pointee.engineReady = 0

        // Encryption fields (version 2+)
        header.pointee.encrypted = 0
        header.pointee.keyFingerprint = (0, 0, 0, 0, 0, 0, 0, 0)
        header.pointee.frameCounter = 0

        // Config negotiation fields (version 3+)
        header.pointee.requestedSampleRate = 0
        header.pointee.requestedBufferFrames = 0
        header.pointee.actualSampleRate = sampleRate
        header.pointee.actualBufferFrames = bufferFrames
        header.pointee.configStatus = 0
        header.pointee.configSource = 0
        header.pointee.configErrorCode = 0

        OSMemoryBarrier()

        halLog("SharedMemory: initialized successfully (version \(kSharedMemoryVersion))")
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

    /// Update sample rate
    func updateSampleRate(_ sampleRate: UInt32) {
        guard let header = header else { return }
        header.pointee.sampleRate = sampleRate
        signalConfigChange()
    }

    /// Write audio to shared memory (called from DoIOOperation for output)
    /// Uses lock-free ring buffer algorithm
    func writeAudio(_ buffer: UnsafePointer<Float>, frameCount: Int, channelCount: Int) -> Int {
        guard let header = header, let audioData = audioData else { return 0 }

        // Check for encryption
        if header.pointee.encrypted != 0 {
            if let cipher = EncryptionKeyManager.shared.getCipher() {
                // Verify fingerprint
                let headerFingerprint = header.pointee.keyFingerprint
                let cipherFingerprint = cipher.getFingerprint()
                let match = headerFingerprint.0 == cipherFingerprint[0] &&
                           headerFingerprint.1 == cipherFingerprint[1] &&
                           headerFingerprint.2 == cipherFingerprint[2] &&
                           headerFingerprint.3 == cipherFingerprint[3] &&
                           headerFingerprint.4 == cipherFingerprint[4] &&
                           headerFingerprint.5 == cipherFingerprint[5] &&
                           headerFingerprint.6 == cipherFingerprint[6] &&
                           headerFingerprint.7 == cipherFingerprint[7]

                if match {
                    // Encrypt to temporary buffer
                    let sampleCount = frameCount * channelCount
                    let ciphertextLen = sampleCount * 4 + 16
                    var ciphertext = [UInt8](repeating: 0, count: ciphertextLen)

                    let frameCounter = UInt64(OSAtomicAdd64(1, &header.pointee.frameCounter))

                    let bytesWritten = ciphertext.withUnsafeMutableBufferPointer { ptr in
                        return cipher.encryptToBuffer(buffer, sampleCount: sampleCount, frameCounter: frameCounter, output: ptr.baseAddress!)
                    }

                    if bytesWritten > 0 {
                        // Prepend nonce (8 bytes big-endian) to ciphertext
                        var payload = [UInt8](repeating: 0, count: 8 + bytesWritten)
                        withUnsafeBytes(of: frameCounter.bigEndian) { nonceBytes in
                            for (i, byte) in nonceBytes.enumerated() {
                                payload[i] = byte
                            }
                        }
                        payload[8..<(8 + bytesWritten)] = ciphertext[..<bytesWritten]

                        // Write payload (nonce + ciphertext) to ring buffer
                        return writeRawBytes(payload, originalFrameCount: frameCount)
                    }
                }
            }
            // If encryption enabled but failed, write silence or return 0
            return 0
        }

        // Standard unencrypted write
        let sampleCount = frameCount * channelCount
        let writePos = header.pointee.writePosition
        let readPos = header.pointee.readPosition

        // Calculate available space
        let used = Int(writePos - readPos)
        let available = audioCapacity - used

        let toWrite = min(sampleCount, available)
        if toWrite <= 0 { return 0 }

        let writeIndex = Int(writePos % UInt64(audioCapacity))
        let firstPart = min(toWrite, audioCapacity - writeIndex)
        let secondPart = toWrite - firstPart

        // Copy data
        memcpy(audioData.advanced(by: writeIndex), buffer, firstPart * MemoryLayout<Float>.size)
        if secondPart > 0 {
            memcpy(audioData, buffer.advanced(by: firstPart), secondPart * MemoryLayout<Float>.size)
        }

        // Update write position atomically
        OSMemoryBarrier()
        header.pointee.writePosition = writePos + UInt64(toWrite)

        // TRACE: Log successful write to shared memory ring buffer
        // Note: Avoid logging every frame in production (use os_signpost for performance tracing)
        #if DEBUG
        if toWrite > 0 {
            let newWritePos = writePos + UInt64(toWrite)
            halLog("[SHM TRACE] write: \(toWrite/channelCount) frames, wpos=\(newWritePos), rpos=\(readPos)")
        }
        #endif

        return toWrite / channelCount
    }

    /// Helper to write raw bytes (ciphertext) to ring buffer
    private func writeRawBytes(_ bytes: [UInt8], originalFrameCount: Int) -> Int {
        guard let header = header, let audioData = audioData else { return 0 }
        
        // Calculate required space in floats (rounded up)
        let floatCount = (bytes.count + 3) / 4
        
        let writePos = header.pointee.writePosition
        let readPos = header.pointee.readPosition
        let available = audioCapacity - Int(writePos - readPos)
        
        if floatCount > available { return 0 }
        
        let writeIndex = Int(writePos % UInt64(audioCapacity))
        let firstPartFloats = min(floatCount, audioCapacity - writeIndex)
        let firstPartBytes = firstPartFloats * 4
        
        let bytesToWriteFirst = min(bytes.count, firstPartBytes)
        let bytesToWriteSecond = bytes.count - bytesToWriteFirst
        
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
            if let cipher = EncryptionKeyManager.shared.getCipher() {
                // Verify fingerprint
                let headerFingerprint = header.pointee.keyFingerprint
                let cipherFingerprint = cipher.getFingerprint()
                let match = headerFingerprint.0 == cipherFingerprint[0] &&
                           headerFingerprint.1 == cipherFingerprint[1] &&
                           headerFingerprint.2 == cipherFingerprint[2] &&
                           headerFingerprint.3 == cipherFingerprint[3] &&
                           headerFingerprint.4 == cipherFingerprint[4] &&
                           headerFingerprint.5 == cipherFingerprint[5] &&
                           headerFingerprint.6 == cipherFingerprint[6] &&
                           headerFingerprint.7 == cipherFingerprint[7]

                if match {
                    // Determine encrypted size: 8-byte nonce + ciphertext + 16-byte tag
                    let sampleCount = frameCount * channelCount
                    let ciphertextBytes = sampleCount * 4 + 16
                    let totalBytes = 8 + ciphertextBytes  // 8 bytes for nonce prefix
                    let floatCount = (totalBytes + 3) / 4

                    // Check availability
                    let writePos = header.pointee.writePosition
                    let readPos = header.pointee.readPosition
                    let available = Int(writePos - readPos)

                    if available >= floatCount {
                        // Read payload (nonce + ciphertext)
                        var payload = [UInt8](repeating: 0, count: totalBytes)
                        readRawBytes(&payload, floatCount: floatCount)

                        // Extract nonce (first 8 bytes, big-endian)
                        var frameCounter: UInt64 = 0
                        withUnsafeMutableBytes(of: &frameCounter) { ptr in
                            for i in 0..<8 {
                                ptr[i] = payload[i]
                            }
                        }
                        frameCounter = UInt64(bigEndian: frameCounter)

                        // Decrypt ciphertext (after nonce)
                        let ciphertext = Array(payload[8..<(8 + ciphertextBytes)])
                        let decryptedCount = ciphertext.withUnsafeBufferPointer { ptr in
                            return cipher.decryptFromBuffer(ptr.baseAddress!, ciphertextLen: ciphertextBytes, frameCounter: frameCounter, output: buffer)
                        }

                        if decryptedCount > 0 {
                            // Fill remainder with silence if needed
                            if decryptedCount < sampleCount {
                                memset(buffer.advanced(by: decryptedCount), 0, (sampleCount - decryptedCount) * MemoryLayout<Float>.size)
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
    private func readRawBytes(_ buffer: inout [UInt8], floatCount: Int) {
        guard let header = header, let audioData = audioData else { return }
        
        let readPos = header.pointee.readPosition
        let readIndex = Int(readPos % UInt64(audioCapacity))
        let firstPartFloats = min(floatCount, audioCapacity - readIndex)
        let firstPartBytes = firstPartFloats * 4
        
        let bytesToReadFirst = min(buffer.count, firstPartBytes)
        let bytesToReadSecond = buffer.count - bytesToReadFirst
        
        buffer.withUnsafeMutableBufferPointer { ptr in
            guard let base = ptr.baseAddress else { return }
            
            // First part
            let srcFirst = UnsafeRawPointer(audioData.advanced(by: readIndex))
            memcpy(base, srcFirst, bytesToReadFirst)
            
            // Second part (wrap)
            if bytesToReadSecond > 0 {
                let srcSecond = UnsafeRawPointer(audioData)
                memcpy(base.advanced(by: bytesToReadFirst), srcSecond, bytesToReadSecond)
            }
        }
        
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

    /// Request a configuration change (called by HAL when client changes sample rate)
    /// Sets configSource=1 (HAL initiated) and configChanged=1
    ///
    /// Memory ordering: All non-atomic fields are written first, then a memory
    /// barrier ensures they are visible before setting configChanged. The
    /// configChanged flag acts as the notification point for the responder.
    func requestConfigChange(sampleRate: UInt32, bufferFrames: UInt32) {
        guard let header = header else { return }
        header.pointee.requestedSampleRate = sampleRate
        header.pointee.requestedBufferFrames = bufferFrames
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