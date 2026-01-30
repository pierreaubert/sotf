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

/// Legacy shared memory file path (for backwards compatibility)
private let kLegacySharedMemoryPath = "/tmp/sotf-audio-shm"

/// Get the secure shared memory path for the current console user
/// This is called from the HAL driver running as _coreaudiod
private func getSecureSharedMemoryPath() -> String {
    // Get the console user (the human logged in, not _coreaudiod)
    var uid: uid_t = 0
    var gid: gid_t = 0

    if let username = SCDynamicStoreCopyConsoleUser(nil, &uid, &gid) as String? {
        // Use the user's TMPDIR if available, otherwise use UID-based path
        // Note: We can't easily get another user's TMPDIR, so use UID-based path
        let userPath = "/tmp/sotf-\(uid)/audio.shm"

        // Create the directory if it doesn't exist
        // Note: Running as _coreaudiod, we cannot set ownership to the console user.
        // The directory will be owned by _coreaudiod with permissions allowing
        // both _coreaudiod and the user to access it.
        let dirPath = "/tmp/sotf-\(uid)"
        var isDir: ObjCBool = false
        if !FileManager.default.fileExists(atPath: dirPath, isDirectory: &isDir) {
            do {
                // Create directory with permissions that allow both _coreaudiod and user access
                // 0777 allows anyone to read/write/execute (files inside will have restricted perms)
                try FileManager.default.createDirectory(atPath: dirPath, withIntermediateDirectories: true, attributes: [
                    .posixPermissions: 0o777
                ])
            } catch {
                halLog("Failed to create secure directory: \(error)")
                // Fall back to legacy path
                return kLegacySharedMemoryPath
            }
        } else {
            // Directory exists, ensure permissions are correct (0777)
            // This fixes the issue where a previous version created it with restrictive permissions (0770)
            do {
                try FileManager.default.setAttributes([.posixPermissions: 0o777], ofItemAtPath: dirPath)
            } catch {
                halLog("Failed to update directory permissions: \(error)")
            }
        }

        return userPath
    }

    // No console user, fall back to legacy path
    halLog("No console user found, using legacy shared memory path")
    return kLegacySharedMemoryPath
}

/// Get the shared memory path (secure if possible, legacy as fallback)
private func getSharedMemoryPath() -> String {
    let securePath = getSecureSharedMemoryPath()

    // If secure path exists, use it
    if FileManager.default.fileExists(atPath: securePath) {
        return securePath
    }

    // If legacy path exists, use it (backwards compatibility)
    if FileManager.default.fileExists(atPath: kLegacySharedMemoryPath) {
        return kLegacySharedMemoryPath
    }

    // Create new secure path
    return securePath
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
        return sharedMemory != nil && header?.pointee.magic == kSharedMemoryMagic
    }

    /// Whether the Rust engine is connected
    var engineReady: Bool {
        return header?.pointee.engineReady != 0
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

        return toWrite / channelCount
    }

    /// Read audio from shared memory (called from DoIOOperation for input)
    /// Uses lock-free ring buffer algorithm
    func readAudio(_ buffer: UnsafeMutablePointer<Float>, frameCount: Int, channelCount: Int) -> Int {
        guard let header = header, let audioData = audioData else {
            // Fill with silence
            memset(buffer, 0, frameCount * channelCount * MemoryLayout<Float>.size)
            return 0
        }

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

        return toRead / channelCount
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
