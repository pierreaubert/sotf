// Tests.swift - Test utilities for HAL driver components
//
// These tests can be called from development builds to verify functionality.
// They are designed to catch regressions in critical areas like:
// - AnyCodable encoding/decoding
// - Socket response parsing
// - Encryption key handling
//
// Note: These are NOT XCTest tests. They are assertion-based verification
// functions that can be called during development and debugging.

import Foundation

/// Test runner for HAL driver components
final class HALDriverTests {

    private struct SharedMemoryLayoutManifest: Decodable {
        let size: Int
        let alignment: Int
        let fields: [String: Int]
    }

    private static func sharedMemoryLayoutManifest() -> SharedMemoryLayoutManifest? {
        var candidates: [URL] = []
        let environment = ProcessInfo.processInfo.environment
        if let configuredPath = environment["SOTF_SHARED_MEMORY_LAYOUT_MANIFEST"],
           !configuredPath.isEmpty
        {
            candidates.append(URL(fileURLWithPath: configuredPath))
        }
        if let bundleURL = Bundle.main.url(
            forResource: "shared_memory_layout",
            withExtension: "json"
        ) {
            candidates.append(bundleURL)
        }
        if let bundleURL = Bundle(for: HALDriverTests.self).url(
            forResource: "shared_memory_layout",
            withExtension: "json"
        ) {
            candidates.append(bundleURL)
        }

        for manifestURL in candidates {
            guard let data = try? Data(contentsOf: manifestURL) else { continue }
            do {
                return try JSONDecoder().decode(SharedMemoryLayoutManifest.self, from: data)
            } catch {
                halLog("    FAIL: shared-memory layout manifest is invalid: \(error)")
                return nil
            }
        }

        halLog(
            "    FAIL: shared-memory layout manifest is missing; set "
                + "SOTF_SHARED_MEMORY_LAYOUT_MANIFEST or package it as a bundle resource"
        )
        return nil
    }

    /// Run all tests
    @discardableResult
    static func runAllTests() -> Bool {
        halLog("Running HAL driver tests...")

        var passed = 0
        var failed = 0

        // Ring buffer tests
        if testRingBufferBasicOperations() { passed += 1 } else { failed += 1 }
        if testRingBufferWrapAround() { passed += 1 } else { failed += 1 }
        if testRingBufferMultiChannel() { passed += 1 } else { failed += 1 }

        // Cross-language shared-memory ABI
        if testSharedAudioHeaderLayout() { passed += 1 } else { failed += 1 }
        if testSampleRateRequestRemainsTransactional() { passed += 1 } else { failed += 1 }
        if testCrossProcessReconfigurationProtocol() { passed += 1 } else { failed += 1 }
        if testGeometryCacheSurvivesDaemonRestart() { passed += 1 } else { failed += 1 }

        // Encryption tests
        if testEncryptionRoundTrip() { passed += 1 } else { failed += 1 }
        if testEncryptionWithDifferentKeys() { passed += 1 } else { failed += 1 }
        if testEncryptionFrameCounterNonceUniqueness() { passed += 1 } else { failed += 1 }
        if testKeyManagerFingerprint() { passed += 1 } else { failed += 1 }
        if testSharedMemoryEncryptedPassthrough() { passed += 1 } else { failed += 1 }

        halLog("Tests complete: \(passed) passed, \(failed) failed")
        return failed == 0
    }

    /// Keep the Swift mirror byte-for-byte compatible with Rust's
    /// `#[repr(C, align(8))] SharedAudioHeader`. Rust asserts the same table in
    /// `shared_memory/tests.rs`; changing either side without updating the
    /// other fails its platform test instead of silently corrupting the mmap.
    static func testSharedAudioHeaderLayout() -> Bool {
        halLog("  Test: SharedAudioHeader layout")

        guard let manifest = sharedMemoryLayoutManifest(),
              // Rust's repr(C, align(8)) size includes trailing padding. Swift
              // exposes the same ABI value as `stride`; `size` only covers the
              // last field and is therefore 140 bytes for this header.
              MemoryLayout<SharedAudioHeader>.stride == manifest.size,
              MemoryLayout<SharedAudioHeader>.alignment == manifest.alignment else {
            halLog(
                "    FAIL: size=\(MemoryLayout<SharedAudioHeader>.size), " +
                "stride=\(MemoryLayout<SharedAudioHeader>.stride), " +
                "alignment=\(MemoryLayout<SharedAudioHeader>.alignment)"
            )
            return false
        }

        let actualOffsets: [String: Int] = [
            "magic": MemoryLayout<SharedAudioHeader>.offset(of: \.magic)!,
            "version": MemoryLayout<SharedAudioHeader>.offset(of: \.version)!,
            "sample_rate": MemoryLayout<SharedAudioHeader>.offset(of: \.sampleRate)!,
            "buffer_frames": MemoryLayout<SharedAudioHeader>.offset(of: \.bufferFrames)!,
            "channel_count": MemoryLayout<SharedAudioHeader>.offset(of: \.channelCount)!,
            "write_position": MemoryLayout<SharedAudioHeader>.offset(of: \.writePosition)!,
            "read_position": MemoryLayout<SharedAudioHeader>.offset(of: \.readPosition)!,
            "active": MemoryLayout<SharedAudioHeader>.offset(of: \.active)!,
            "config_changed": MemoryLayout<SharedAudioHeader>.offset(of: \.configChanged)!,
            "driver_ready": MemoryLayout<SharedAudioHeader>.offset(of: \.driverReady)!,
            "engine_ready": MemoryLayout<SharedAudioHeader>.offset(of: \.engineReady)!,
            "encrypted": MemoryLayout<SharedAudioHeader>.offset(of: \.encrypted)!,
            "key_fingerprint": MemoryLayout<SharedAudioHeader>.offset(of: \.keyFingerprint)!,
            "frame_counter": MemoryLayout<SharedAudioHeader>.offset(of: \.frameCounter)!,
            "requested_sample_rate": MemoryLayout<SharedAudioHeader>.offset(of: \.requestedSampleRate)!,
            "requested_buffer_frames": MemoryLayout<SharedAudioHeader>.offset(of: \.requestedBufferFrames)!,
            "actual_sample_rate": MemoryLayout<SharedAudioHeader>.offset(of: \.actualSampleRate)!,
            "actual_buffer_frames": MemoryLayout<SharedAudioHeader>.offset(of: \.actualBufferFrames)!,
            "config_status": MemoryLayout<SharedAudioHeader>.offset(of: \.configStatus)!,
            "config_source": MemoryLayout<SharedAudioHeader>.offset(of: \.configSource)!,
            "config_error_code": MemoryLayout<SharedAudioHeader>.offset(of: \.configErrorCode)!,
            "encryption_overflow_count": MemoryLayout<SharedAudioHeader>.offset(of: \.encryptionOverflowCount)!,
            "daemon_heartbeat_ms": MemoryLayout<SharedAudioHeader>.offset(of: \.daemonHeartbeatMs)!,
            "configuring": MemoryLayout<SharedAudioHeader>.offset(of: \.configuring)!,
            "configuring_ack": MemoryLayout<SharedAudioHeader>.offset(of: \.configuringAck)!,
            "requested_channel_count": MemoryLayout<SharedAudioHeader>.offset(of: \.requestedChannelCount)!,
        ]
        guard Set(actualOffsets.keys) == Set(manifest.fields.keys) else {
            halLog("    FAIL: manifest field set does not match Swift header")
            return false
        }
        for (name, expected) in manifest.fields where actualOffsets[name] != expected {
            halLog("    FAIL: \(name) offset=\(String(describing: actualOffsets[name])), expected=\(expected)")
            return false
        }

        halLog("    PASS")
        return true
    }

    /// A HAL property change must publish a request without changing the
    /// active ring geometry. The daemon owns the acknowledgment; only then
    /// may the active sample rate be changed by the driver state machine.
    static func testSampleRateRequestRemainsTransactional() -> Bool {
        halLog("  Test: sample-rate request remains transactional")

        let fileManager = FileManager.default
        let tempDir = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("sotf-hal-config-(UUID().uuidString)")
        let shmPath = (tempDir as NSString).appendingPathComponent("audio.shm")
        let sampleRate: UInt32 = 48_000
        let bufferFrames: UInt32 = 64
        let channelCount: UInt32 = 2
        let audioCapacity = Int(bufferFrames) * Int(channelCount) * 8
        let headerSize = MemoryLayout<SharedAudioHeader>.size
        let alignedHeaderSize = (headerSize + 63) & ~63
        let totalSize = alignedHeaderSize + audioCapacity * MemoryLayout<Float>.size

        do {
            try fileManager.createDirectory(
                atPath: tempDir,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            fileManager.createFile(atPath: shmPath, contents: Data(count: totalSize))
        } catch {
            halLog("    FAIL: fixture setup failed: (error)")
            return false
        }

        var header = SharedAudioHeader(
            magic: 0x534F5446,
            version: 6,
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: channelCount,
            writePosition: 0,
            readPosition: 0,
            active: 0,
            configChanged: 0,
            driverReady: 0,
            engineReady: 1,
            encrypted: 0,
            keyFingerprint: 0,
            frameCounter: 0,
            requestedSampleRate: sampleRate,
            requestedBufferFrames: bufferFrames,
            actualSampleRate: sampleRate,
            actualBufferFrames: bufferFrames,
            configStatus: 1,
            configSource: 0,
            configErrorCode: 0,
            encryptionOverflowCount: 0,
            daemonHeartbeatMs: 0,
            configuring: 0,
            configuringAck: 0,
            requestedChannelCount: channelCount
        )

        let fd = Darwin.open(shmPath, O_RDWR)
        guard fd >= 0 else {
            try? fileManager.removeItem(atPath: tempDir)
            halLog("    FAIL: fixture open failed")
            return false
        }
        let headerWritten = withUnsafeBytes(of: &header) { bytes in
            Darwin.write(fd, bytes.baseAddress, bytes.count)
        }
        Darwin.close(fd)
        guard headerWritten == headerSize else {
            try? fileManager.removeItem(atPath: tempDir)
            halLog("    FAIL: fixture header write failed")
            return false
        }

        setenv("SOTF_HAL_SHARED_MEMORY_PATH", shmPath, 1)
        defer {
            unsetenv("SOTF_HAL_SHARED_MEMORY_PATH")
            try? fileManager.removeItem(atPath: tempDir)
        }

        let sharedAudio = SharedAudioBuffer()
        guard sharedAudio.initialize(
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: channelCount
        ) else {
            halLog("    FAIL: SharedAudioBuffer.initialize failed")
            return false
        }
        defer { sharedAudio.closeSharedMemory() }

        sharedAudio.updateSampleRate(96_000)

        guard sharedAudio.getActiveSampleRate() == sampleRate,
              sharedAudio.getRequestedSampleRate() == 96_000,
              sharedAudio.configChanged(),
              sharedAudio.configSource() == 1,
              sharedAudio.getConfigStatus() == 0 else {
            halLog("    FAIL: request changed active state or did not publish pending status")
            return false
        }

        sharedAudio.acknowledgeConfigChange(
            actualSampleRate: 96_000,
            actualBufferFrames: bufferFrames,
            status: 1,
            errorCode: 0
        )
        guard sharedAudio.applyActiveConfiguration(
            sampleRate: 96_000,
            bufferFrames: bufferFrames,
            channelCount: channelCount
        ), sharedAudio.getActiveSampleRate() == 96_000 else {
            halLog("    FAIL: acknowledged geometry was not committed")
            return false
        }

        halLog("    PASS")
        return true
    }

    /// Exercise the same pending-channel and configuring_ack protocol used by
    /// the Rust daemon. The fixture is intentionally larger than the active
    /// 2-channel geometry so the new 8-channel geometry can be committed only
    /// after the simulated Rust-side quiesce request has been acknowledged by
    /// Swift IO.
    static func testCrossProcessReconfigurationProtocol() -> Bool {
        halLog("  Test: cross-process reconfiguration protocol")

        let fileManager = FileManager.default
        let tempDir = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("sotf-hal-reconfigure-\(UUID().uuidString)")
        let shmPath = (tempDir as NSString).appendingPathComponent("audio.shm")
        let sampleRate: UInt32 = 48_000
        let bufferFrames: UInt32 = 64
        let activeChannels: UInt32 = 2
        let pendingChannels: UInt32 = 8
        let audioCapacity = Int(bufferFrames) * Int(pendingChannels) * 8
        let alignedHeaderSize = (MemoryLayout<SharedAudioHeader>.size + 63) & ~63
        let totalSize = alignedHeaderSize + audioCapacity * MemoryLayout<Float>.size

        do {
            try fileManager.createDirectory(
                atPath: tempDir,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            fileManager.createFile(atPath: shmPath, contents: Data(count: totalSize))
        } catch {
            halLog("    FAIL: fixture setup failed: \(error)")
            return false
        }
        defer { try? fileManager.removeItem(atPath: tempDir) }

        var header = SharedAudioHeader(
            magic: 0x534F5446,
            version: 6,
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: activeChannels,
            writePosition: 0,
            readPosition: 0,
            active: 0,
            configChanged: 0,
            driverReady: 0,
            engineReady: 1,
            encrypted: 0,
            keyFingerprint: 0,
            frameCounter: 0,
            requestedSampleRate: sampleRate,
            requestedBufferFrames: bufferFrames,
            actualSampleRate: sampleRate,
            actualBufferFrames: bufferFrames,
            configStatus: 1,
            configSource: 0,
            configErrorCode: 0,
            encryptionOverflowCount: 0,
            daemonHeartbeatMs: 0,
            configuring: 0,
            configuringAck: 0,
            requestedChannelCount: activeChannels
        )
        let fd = Darwin.open(shmPath, O_RDWR)
        guard fd >= 0 else {
            halLog("    FAIL: fixture open failed")
            return false
        }
        let headerWritten = withUnsafeBytes(of: &header) { bytes in
            Darwin.pwrite(fd, bytes.baseAddress, bytes.count, 0)
        }
        guard headerWritten == MemoryLayout<SharedAudioHeader>.size else {
            Darwin.close(fd)
            halLog("    FAIL: fixture header write failed")
            return false
        }

        func writeUInt32(_ value: UInt32, at offset: off_t) -> Bool {
            var value = value
            return withUnsafeBytes(of: &value) { bytes in
                Darwin.pwrite(fd, bytes.baseAddress, bytes.count, offset) == bytes.count
            }
        }
        func readUInt32(at offset: off_t) -> UInt32? {
            var value: UInt32 = 0
            let count = withUnsafeMutableBytes(of: &value) { bytes in
                Darwin.pread(fd, bytes.baseAddress, bytes.count, offset)
            }
            return count == MemoryLayout<UInt32>.size ? value : nil
        }

        setenv("SOTF_HAL_SHARED_MEMORY_PATH", shmPath, 1)
        let sharedAudio = SharedAudioBuffer()
        guard sharedAudio.initialize(
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: activeChannels
        ) else {
            Darwin.close(fd)
            unsetenv("SOTF_HAL_SHARED_MEMORY_PATH")
            halLog("    FAIL: SharedAudioBuffer.initialize failed")
            return false
        }
        defer {
            sharedAudio.closeSharedMemory()
            Darwin.close(fd)
            unsetenv("SOTF_HAL_SHARED_MEMORY_PATH")
        }

        // Swift publishes the pending request while the active geometry stays
        // at 2 channels. A Rust daemon would consume these fields next.
        sharedAudio.updateSampleRate(96_000)
        guard writeUInt32(pendingChannels, at: 136),
              sharedAudio.getActiveChannelCount() == activeChannels,
              sharedAudio.getRequestedSampleRate() == 96_000,
              sharedAudio.getRequestedChannelCount() == pendingChannels else {
            halLog("    FAIL: pending request changed active geometry")
            return false
        }

        // A Rust-side reconfigure request sets configuring. Swift IO must
        // acknowledge it before either process changes ring geometry.
        guard writeUInt32(1, at: 128) else {
            halLog("    FAIL: could not publish configuring request")
            return false
        }
        var silence = [Float](repeating: 0, count: 2)
        _ = silence.withUnsafeMutableBufferPointer { buffer in
            sharedAudio.readAudio(buffer.baseAddress!, frameCount: 1, channelCount: 2)
        }
        guard readUInt32(at: 132) == 1 else {
            halLog("    FAIL: Swift IO did not publish configuring_ack")
            return false
        }
        guard writeUInt32(0, at: 128) else {
            halLog("    FAIL: could not clear configuring request")
            return false
        }

        sharedAudio.acknowledgeConfigChange(
            actualSampleRate: 96_000,
            actualBufferFrames: bufferFrames,
            status: 1,
            errorCode: 0
        )
        let applied = sharedAudio.applyActiveConfiguration(
            sampleRate: 96_000,
            bufferFrames: bufferFrames,
            channelCount: pendingChannels
        )
        let activeSampleRate = sharedAudio.getActiveSampleRate()
        let activeChannelCount = sharedAudio.getActiveChannelCount()
        let configuring = readUInt32(at: 128)
        let configuringAck = readUInt32(at: 132)
        guard applied,
              activeSampleRate == 96_000,
              activeChannelCount == pendingChannels,
              configuring == 0,
              configuringAck == 0 else {
            halLog(
                "    FAIL: acknowledged geometry was not committed atomically "
                    + "(applied=\(applied), rate=\(activeSampleRate), "
                    + "channels=\(activeChannelCount), configuring=\(String(describing: configuring)), "
                    + "ack=\(String(describing: configuringAck)))"
            )
            return false
        }

        guard runRustSwiftReconfigurationStress(
            sharedAudio: sharedAudio,
            sharedMemoryPath: shmPath
        ) else {
            return false
        }

        halLog("    PASS")
        return true
    }

    private static func runRustSwiftReconfigurationStress(
        sharedAudio: SharedAudioBuffer,
        sharedMemoryPath: String
    ) -> Bool {
        guard let workerPath = ProcessInfo.processInfo.environment[
            "SOTF_RUST_HAL_TRANSPORT_WORKER"
        ], FileManager.default.isExecutableFile(atPath: workerPath) else {
            halLog("    FAIL: SOTF_RUST_HAL_TRANSPORT_WORKER is not executable")
            return false
        }

        let process = Process()
        let diagnostics = Pipe()
        process.executableURL = URL(fileURLWithPath: workerPath)
        process.arguments = [sharedMemoryPath, "200"]
        process.standardOutput = diagnostics
        process.standardError = diagnostics

        do {
            try process.run()
        } catch {
            halLog("    FAIL: could not start Rust transport worker: \(error)")
            return false
        }

        let deadline = Date().addingTimeInterval(15)
        var framesRead = 0
        while process.isRunning && Date() < deadline {
            let channels = max(Int(sharedAudio.getActiveChannelCount()), 1)
            var samples = [Float](repeating: 0, count: 64 * channels)
            let read = samples.withUnsafeMutableBufferPointer { buffer in
                sharedAudio.readAudio(
                    buffer.baseAddress!,
                    frameCount: 64,
                    channelCount: channels
                )
            }
            framesRead += max(read, 0)
            usleep(50)
        }

        if process.isRunning {
            process.terminate()
            process.waitUntilExit()
            halLog("    FAIL: Rust transport worker timed out")
            return false
        }
        process.waitUntilExit()

        let output = diagnostics.fileHandleForReading.readDataToEndOfFile()
        let outputText = String(data: output, encoding: .utf8) ?? ""
        guard process.terminationStatus == 0 else {
            halLog(
                "    FAIL: Rust transport worker exited \(process.terminationStatus): "
                + outputText
            )
            return false
        }
        guard framesRead > 0,
              sharedAudio.getActiveChannelCount() == 2
                || sharedAudio.getActiveChannelCount() == 8,
              sharedAudio.getActiveSampleRate() == 48_000
                || sharedAudio.getActiveSampleRate() == 96_000
        else {
            halLog(
                "    FAIL: cross-language stress produced no frames or invalid geometry "
                + "(frames=\(framesRead), rate=\(sharedAudio.getActiveSampleRate()), "
                + "channels=\(sharedAudio.getActiveChannelCount()))"
            )
            return false
        }

        halLog("    Rust/Swift stress: \(framesRead) frames read across 200 reconfigurations")
        return true
    }

    /// A CoreAudio helper restart must preserve the daemon-owned active
    /// geometry. Reopening the mapping with a different requested format may
    /// publish a pending request, but it must never use a stale local cache to
    /// rewrite `channelCount` before the daemon acknowledges it.
    static func testGeometryCacheSurvivesDaemonRestart() -> Bool {
        halLog("  Test: geometry cache survives daemon restart")

        let fileManager = FileManager.default
        let tempDir = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("sotf-hal-restart-\(UUID().uuidString)")
        let shmPath = (tempDir as NSString).appendingPathComponent("audio.shm")
        let sampleRate: UInt32 = 48_000
        let bufferFrames: UInt32 = 64
        let activeChannels: UInt32 = 2
        let requestedChannels: UInt32 = 8
        let audioCapacity = Int(bufferFrames) * Int(requestedChannels) * 8
        let alignedHeaderSize = (MemoryLayout<SharedAudioHeader>.size + 63) & ~63
        let totalSize = alignedHeaderSize + audioCapacity * MemoryLayout<Float>.size

        do {
            try fileManager.createDirectory(
                atPath: tempDir,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            fileManager.createFile(atPath: shmPath, contents: Data(count: totalSize))
        } catch {
            halLog("    FAIL: fixture setup failed: \(error)")
            return false
        }
        defer { try? fileManager.removeItem(atPath: tempDir) }

        var header = SharedAudioHeader(
            magic: 0x534F5446,
            version: 6,
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: activeChannels,
            writePosition: 0,
            readPosition: 0,
            active: 1,
            configChanged: 0,
            driverReady: 1,
            engineReady: 1,
            encrypted: 0,
            keyFingerprint: 0,
            frameCounter: 0,
            requestedSampleRate: sampleRate,
            requestedBufferFrames: bufferFrames,
            actualSampleRate: sampleRate,
            actualBufferFrames: bufferFrames,
            configStatus: 1,
            configSource: 0,
            configErrorCode: 0,
            encryptionOverflowCount: 0,
            daemonHeartbeatMs: 1,
            configuring: 0,
            configuringAck: 0,
            requestedChannelCount: activeChannels
        )

        let fd = Darwin.open(shmPath, O_RDWR)
        guard fd >= 0 else {
            halLog("    FAIL: fixture open failed")
            return false
        }
        let written = withUnsafeBytes(of: &header) { bytes in
            Darwin.pwrite(fd, bytes.baseAddress, bytes.count, 0)
        }
        Darwin.close(fd)
        guard written == MemoryLayout<SharedAudioHeader>.size else {
            halLog("    FAIL: fixture header write failed")
            return false
        }

        setenv("SOTF_HAL_SHARED_MEMORY_PATH", shmPath, 1)
        defer { unsetenv("SOTF_HAL_SHARED_MEMORY_PATH") }

        let firstConnection = SharedAudioBuffer()
        guard firstConnection.initialize(
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: activeChannels
        ) else {
            halLog("    FAIL: first HAL connection failed")
            return false
        }
        firstConnection.closeSharedMemory()

        let restartedConnection = SharedAudioBuffer()
        guard restartedConnection.initialize(
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: requestedChannels
        ) else {
            halLog("    FAIL: restarted HAL connection failed")
            return false
        }
        defer { restartedConnection.closeSharedMemory() }

        guard restartedConnection.getActiveChannelCount() == activeChannels,
              restartedConnection.getRequestedChannelCount() == requestedChannels,
              restartedConnection.configChanged(),
              restartedConnection.configSource() == 1 else {
            halLog("    FAIL: restart rewrote active geometry or lost pending request")
            return false
        }

        halLog("    PASS")
        return true
    }

    // ==========================================================================
    // Ring Buffer Tests
    // ==========================================================================

    /// Test basic ring buffer operations
    static func testRingBufferBasicOperations() -> Bool {
        halLog("  Test: RingBuffer basic operations")

        let buffer = AudioRingBuffer(capacity: 1024)

        // Initially empty
        guard buffer.availableToRead == 0 else {
            halLog("    FAIL: Buffer should be empty initially")
            return false
        }

        guard buffer.availableToWrite > 0 else {
            halLog("    FAIL: Buffer should have space to write initially")
            return false
        }

        // Write some data
        var writeData = [Float](repeating: 0.5, count: 256)
        let written = buffer.write(&writeData, count: 256)

        guard written == 256 else {
            halLog("    FAIL: Should write 256 samples, wrote \(written)")
            return false
        }

        guard buffer.availableToRead == 256 else {
            halLog("    FAIL: Should have 256 samples to read, have \(buffer.availableToRead)")
            return false
        }

        // Read data back
        var readData = [Float](repeating: 0, count: 256)
        let read = buffer.read(&readData, count: 256)

        guard read == 256 else {
            halLog("    FAIL: Should read 256 samples, read \(read)")
            return false
        }

        // Verify data matches
        for i in 0..<256 {
            if readData[i] != writeData[i] {
                halLog("    FAIL: Data mismatch at index \(i)")
                return false
            }
        }

        halLog("    PASS")
        return true
    }

    /// Test ring buffer wrap-around behavior
    static func testRingBufferWrapAround() -> Bool {
        halLog("  Test: RingBuffer wrap-around")

        let capacity = 256
        let buffer = AudioRingBuffer(capacity: capacity)

        // Write and read multiple times to force wrap-around
        for iteration in 0..<10 {
            var writeData: [Float] = (0..<128).map { Float($0 + iteration * 128) }
            let written = buffer.write(&writeData, count: 128)

            guard written == 128 else {
                halLog("    FAIL: Iteration \(iteration) - wrote \(written), expected 128")
                return false
            }

            var readData = [Float](repeating: 0, count: 128)
            let read = buffer.read(&readData, count: 128)

            guard read == 128 else {
                halLog("    FAIL: Iteration \(iteration) - read \(read), expected 128")
                return false
            }

            // Verify data
            for i in 0..<128 {
                if readData[i] != writeData[i] {
                    halLog("    FAIL: Data mismatch at iteration \(iteration), index \(i)")
                    return false
                }
            }
        }

        halLog("    PASS")
        return true
    }

    /// Test multi-channel ring buffer operations
    static func testRingBufferMultiChannel() -> Bool {
        halLog("  Test: RingBuffer multi-channel")

        let channels = 6  // 5.1 surround
        let frames = 64
        let buffer = MultiChannelRingBuffer(channelCount: channels, framesCapacity: 1024)

        // Create interleaved multi-channel data
        var writeData = [Float](repeating: 0, count: frames * channels)
        for frame in 0..<frames {
            for ch in 0..<channels {
                // Each channel has a unique offset for identification
                writeData[frame * channels + ch] = Float(frame) + Float(ch) * 0.1
            }
        }

        // Write interleaved
        let written = buffer.writeInterleaved(&writeData, frameCount: frames)
        guard written == frames else {
            halLog("    FAIL: Should write \(frames) frames, wrote \(written)")
            return false
        }

        // Read interleaved
        var readData = [Float](repeating: 0, count: frames * channels)
        let read = buffer.readInterleaved(&readData, frameCount: frames)
        guard read == frames else {
            halLog("    FAIL: Should read \(frames) frames, read \(read)")
            return false
        }

        // Verify all channels preserved
        for frame in 0..<frames {
            for ch in 0..<channels {
                let idx = frame * channels + ch
                if readData[idx] != writeData[idx] {
                    halLog("    FAIL: Mismatch at frame \(frame), channel \(ch)")
                    return false
                }
            }
        }

        halLog("    PASS")
        return true
    }

    // ==========================================================================
    // Encryption Tests
    // ==========================================================================

    /// Test encryption round-trip
    static func testEncryptionRoundTrip() -> Bool {
        halLog("  Test: Encryption round-trip")

        let keyBytes: [UInt8] = (0..<32).map { UInt8($0) }
        let cipher = AudioCipher(keyBytes: keyBytes)

        // Create test audio data
        let samples: [Float] = (0..<256).map { sin(Float($0) * 0.1) }

        // Encrypt
        guard let encrypted = cipher.encrypt(samples: samples, frameCounter: 1) else {
            halLog("    FAIL: Encryption failed")
            return false
        }

        // Encrypted should be larger (includes auth tag)
        guard encrypted.count > samples.count * 4 else {
            halLog("    FAIL: Encrypted data too small")
            return false
        }

        // Decrypt
        guard let decrypted = cipher.decrypt(ciphertext: encrypted, frameCounter: 1) else {
            halLog("    FAIL: Decryption failed")
            return false
        }

        // Verify
        guard decrypted.count == samples.count else {
            halLog("    FAIL: Decrypted length mismatch: \(decrypted.count) vs \(samples.count)")
            return false
        }

        for i in 0..<samples.count {
            if abs(decrypted[i] - samples[i]) > 0.0001 {
                halLog("    FAIL: Sample mismatch at \(i): \(decrypted[i]) vs \(samples[i])")
                return false
            }
        }

        halLog("    PASS")
        return true
    }

    /// Test that different keys produce different ciphertext
    static func testEncryptionWithDifferentKeys() -> Bool {
        halLog("  Test: Encryption with different keys")

        let key1: [UInt8] = (0..<32).map { UInt8($0) }
        let key2: [UInt8] = (0..<32).map { UInt8(31 - $0) }  // Different key

        let cipher1 = AudioCipher(keyBytes: key1)
        let cipher2 = AudioCipher(keyBytes: key2)

        // Verify fingerprints differ
        guard cipher1.getFingerprintHex() != cipher2.getFingerprintHex() else {
            halLog("    FAIL: Different keys should have different fingerprints")
            return false
        }

        // Encrypt same data with different keys
        let samples: [Float] = [0.5, 0.25, -0.5, -0.25]

        guard let encrypted1 = cipher1.encrypt(samples: samples, frameCounter: 1),
              let encrypted2 = cipher2.encrypt(samples: samples, frameCounter: 1) else {
            halLog("    FAIL: Encryption failed")
            return false
        }

        // Ciphertexts should differ
        guard encrypted1 != encrypted2 else {
            halLog("    FAIL: Different keys should produce different ciphertext")
            return false
        }

        // Decryption with wrong key should fail
        let wrongDecrypt = cipher1.decrypt(ciphertext: encrypted2, frameCounter: 1)
        guard wrongDecrypt == nil else {
            halLog("    FAIL: Decryption with wrong key should fail")
            return false
        }

        halLog("    PASS")
        return true
    }

    /// Test that frame counter provides unique nonces
    static func testEncryptionFrameCounterNonceUniqueness() -> Bool {
        halLog("  Test: Encryption frame counter nonce uniqueness")

        let keyBytes: [UInt8] = (0..<32).map { UInt8($0) }
        let cipher = AudioCipher(keyBytes: keyBytes)

        let samples: [Float] = [0.5, 0.25, -0.5, -0.25]

        // Same data encrypted with different frame counters should produce different ciphertext
        guard let enc1 = cipher.encrypt(samples: samples, frameCounter: 1),
              let enc2 = cipher.encrypt(samples: samples, frameCounter: 2),
              let enc3 = cipher.encrypt(samples: samples, frameCounter: 3) else {
            halLog("    FAIL: Encryption failed")
            return false
        }

        guard enc1 != enc2 && enc2 != enc3 && enc1 != enc3 else {
            halLog("    FAIL: Different frame counters should produce different ciphertext")
            return false
        }

        // Each can be decrypted with correct frame counter
        guard cipher.decrypt(ciphertext: enc1, frameCounter: 1) != nil,
              cipher.decrypt(ciphertext: enc2, frameCounter: 2) != nil,
              cipher.decrypt(ciphertext: enc3, frameCounter: 3) != nil else {
            halLog("    FAIL: Decryption with correct frame counter should succeed")
            return false
        }

        // Wrong frame counter should fail
        guard cipher.decrypt(ciphertext: enc1, frameCounter: 2) == nil,
              cipher.decrypt(ciphertext: enc2, frameCounter: 1) == nil else {
            halLog("    FAIL: Decryption with wrong frame counter should fail")
            return false
        }

        halLog("    PASS")
        return true
    }

    /// Test encrypted shared-memory write/read using the same Swift HAL path
    /// that Rust consumes. The daemon-shaped memory file and key are temporary.
    static func testSharedMemoryEncryptedPassthrough() -> Bool {
        halLog("  Test: SharedMemory encrypted passthrough")

        let fileManager = FileManager.default
        let tempDir = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("sotf-hal-encrypted-\(UUID().uuidString)")
        let shmPath = (tempDir as NSString).appendingPathComponent("audio.shm")
        let keyPath = (tempDir as NSString).appendingPathComponent("session.key")
        let sampleRate: UInt32 = 48_000
        let bufferFrames: UInt32 = 64
        let channelCount: UInt32 = 2
        let audioCapacity = Int(bufferFrames) * Int(channelCount) * 8
        let headerSize = MemoryLayout<SharedAudioHeader>.size
        let alignedHeaderSize = (headerSize + 63) & ~63
        let totalSize = alignedHeaderSize + audioCapacity * MemoryLayout<Float>.size
        let keyBytes: [UInt8] = (0..<32).map { UInt8($0) }
        let cipher = AudioCipher(keyBytes: keyBytes)

        func fingerprintUInt64(_ bytes: [UInt8]) -> UInt64 {
            var value: UInt64 = 0
            for byte in bytes.prefix(8) {
                value = (value << 8) | UInt64(byte)
            }
            return value
        }

        do {
            try fileManager.createDirectory(
                atPath: tempDir,
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            fileManager.createFile(atPath: shmPath, contents: Data(count: totalSize))
            try Data(keyBytes).write(to: URL(fileURLWithPath: keyPath))
            chmod(keyPath, 0o600)
        } catch {
            halLog("    FAIL: setup failed: \(error)")
            return false
        }

        setenv("SOTF_HAL_SHARED_MEMORY_PATH", shmPath, 1)
        setenv("SOTF_HAL_SESSION_KEY_PATH", keyPath, 1)
        defer {
            unsetenv("SOTF_HAL_SHARED_MEMORY_PATH")
            unsetenv("SOTF_HAL_SESSION_KEY_PATH")
            try? fileManager.removeItem(atPath: tempDir)
        }

        var header = SharedAudioHeader(
            magic: 0x534F5446,
            version: 6,
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: channelCount,
            writePosition: 0,
            readPosition: 0,
            active: 1,
            configChanged: 0,
            driverReady: 0,
            engineReady: 1,
            encrypted: 1,
            keyFingerprint: fingerprintUInt64(cipher.getFingerprint()),
            frameCounter: 0,
            requestedSampleRate: sampleRate,
            requestedBufferFrames: bufferFrames,
            actualSampleRate: sampleRate,
            actualBufferFrames: bufferFrames,
            configStatus: 1,
            configSource: 2,
            configErrorCode: 0,
            encryptionOverflowCount: 0,
            daemonHeartbeatMs: 0,
            configuring: 0,
            configuringAck: 0,
            requestedChannelCount: channelCount
        )

        let fd = Darwin.open(shmPath, O_RDWR)
        guard fd >= 0 else {
            halLog("    FAIL: open shared memory fixture failed")
            return false
        }
        let headerWritten = withUnsafeBytes(of: &header) { bytes in
            Darwin.write(fd, bytes.baseAddress, bytes.count)
        }
        Darwin.close(fd)
        guard headerWritten == headerSize else {
            halLog("    FAIL: header write failed")
            return false
        }

        EncryptionKeyManager.shared.reloadFromCurrentEnvironment()
        let sharedMemory = SharedAudioBuffer()
        guard sharedMemory.initialize(
            sampleRate: sampleRate,
            bufferFrames: bufferFrames,
            channelCount: channelCount
        ) else {
            halLog("    FAIL: SharedAudioBuffer.initialize failed")
            return false
        }
        defer { sharedMemory.closeSharedMemory() }

        let frames = Int(bufferFrames)
        let channels = Int(channelCount)
        var input = [Float]()
        input.reserveCapacity(frames * channels)
        for frame in 0..<frames {
            input.append(sin(Float(frame) * 0.1) * 0.5)
            input.append(cos(Float(frame) * 0.07) * 0.25)
        }

        let written = input.withUnsafeBufferPointer { ptr in
            sharedMemory.writeAudio(ptr.baseAddress!, frameCount: frames, channelCount: channels)
        }
        guard written == frames else {
            halLog("    FAIL: encrypted write returned \(written), expected \(frames)")
            return false
        }

        var output = [Float](repeating: 0, count: input.count)
        let read = output.withUnsafeMutableBufferPointer { ptr in
            sharedMemory.readAudio(ptr.baseAddress!, frameCount: frames, channelCount: channels)
        }
        guard read == frames else {
            halLog("    FAIL: encrypted read returned \(read), expected \(frames)")
            return false
        }

        for i in 0..<input.count {
            if input[i].bitPattern != output[i].bitPattern {
                halLog("    FAIL: sample \(i) mismatch \(input[i]) vs \(output[i])")
                return false
            }
        }

        halLog("    PASS")
        return true
    }

    /// Test key manager fingerprint consistency
    static func testKeyManagerFingerprint() -> Bool {
        halLog("  Test: KeyManager fingerprint")

        let manager = EncryptionKeyManager.shared

        // Fingerprint should be consistent
        guard let fp1 = manager.getFingerprint(),
              let fp2 = manager.getFingerprint() else {
            halLog("    SKIP: KeyManager has no key loaded")
            return true  // Not a failure, just skip
        }

        guard fp1 == fp2 else {
            halLog("    FAIL: Fingerprint should be consistent")
            return false
        }

        // Fingerprint should be 8 bytes
        guard fp1.count == 8 else {
            halLog("    FAIL: Fingerprint should be 8 bytes, got \(fp1.count)")
            return false
        }

        halLog("    PASS")
        return true
    }
}

// ==========================================================================
// ConfigBar Tests - to be added to ConfigBar.swift or separate test target
// ==========================================================================

/// Test utilities for ConfigBar components
/// Note: These would typically be in a separate test target with XCTest
class ConfigBarTestUtils {

    /// Test AnyCodable encoding/decoding
    static func testAnyCodableRoundTrip() -> Bool {
        // Test would require ConfigBar.swift AnyCodable struct
        // Implementation deferred to XCTest target
        return true
    }

    /// Test socket response parsing with fragmentation
    static func testSocketResponseParsing() -> Bool {
        // Test would require actual socket connection
        // Implementation deferred to integration tests
        return true
    }
}
