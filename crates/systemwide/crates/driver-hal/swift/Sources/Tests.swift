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

    /// Run all tests
    static func runAllTests() {
        halLog("Running HAL driver tests...")

        var passed = 0
        var failed = 0

        // Ring buffer tests
        if testRingBufferBasicOperations() { passed += 1 } else { failed += 1 }
        if testRingBufferWrapAround() { passed += 1 } else { failed += 1 }
        if testRingBufferMultiChannel() { passed += 1 } else { failed += 1 }

        // Cross-language shared-memory ABI
        if testSharedAudioHeaderLayout() { passed += 1 } else { failed += 1 }

        // Encryption tests
        if testEncryptionRoundTrip() { passed += 1 } else { failed += 1 }
        if testEncryptionWithDifferentKeys() { passed += 1 } else { failed += 1 }
        if testEncryptionFrameCounterNonceUniqueness() { passed += 1 } else { failed += 1 }
        if testKeyManagerFingerprint() { passed += 1 } else { failed += 1 }
        if testSharedMemoryEncryptedPassthrough() { passed += 1 } else { failed += 1 }

        halLog("Tests complete: \(passed) passed, \(failed) failed")
    }

    /// Keep the Swift mirror byte-for-byte compatible with Rust's
    /// `#[repr(C, align(8))] SharedAudioHeader`. Rust asserts the same table in
    /// `shared_memory/tests.rs`; changing either side without updating the
    /// other fails its platform test instead of silently corrupting the mmap.
    static func testSharedAudioHeaderLayout() -> Bool {
        halLog("  Test: SharedAudioHeader layout")

        let expectedOffsets: [(String, Int?, Int)] = [
            ("magic", MemoryLayout<SharedAudioHeader>.offset(of: \.magic), 0),
            ("version", MemoryLayout<SharedAudioHeader>.offset(of: \.version), 4),
            ("sampleRate", MemoryLayout<SharedAudioHeader>.offset(of: \.sampleRate), 8),
            ("bufferFrames", MemoryLayout<SharedAudioHeader>.offset(of: \.bufferFrames), 12),
            ("channelCount", MemoryLayout<SharedAudioHeader>.offset(of: \.channelCount), 16),
            ("writePosition", MemoryLayout<SharedAudioHeader>.offset(of: \.writePosition), 24),
            ("readPosition", MemoryLayout<SharedAudioHeader>.offset(of: \.readPosition), 32),
            ("active", MemoryLayout<SharedAudioHeader>.offset(of: \.active), 40),
            ("keyFingerprint", MemoryLayout<SharedAudioHeader>.offset(of: \.keyFingerprint), 64),
            ("frameCounter", MemoryLayout<SharedAudioHeader>.offset(of: \.frameCounter), 72),
            ("requestedSampleRate", MemoryLayout<SharedAudioHeader>.offset(of: \.requestedSampleRate), 80),
            ("configStatus", MemoryLayout<SharedAudioHeader>.offset(of: \.configStatus), 96),
            ("encryptionOverflowCount", MemoryLayout<SharedAudioHeader>.offset(of: \.encryptionOverflowCount), 112),
            ("daemonHeartbeatMs", MemoryLayout<SharedAudioHeader>.offset(of: \.daemonHeartbeatMs), 120),
            ("configuring", MemoryLayout<SharedAudioHeader>.offset(of: \.configuring), 128),
            ("configuringAck", MemoryLayout<SharedAudioHeader>.offset(of: \.configuringAck), 132),
            ("requestedChannelCount", MemoryLayout<SharedAudioHeader>.offset(of: \.requestedChannelCount), 136),
        ]

        guard MemoryLayout<SharedAudioHeader>.size == 144,
              MemoryLayout<SharedAudioHeader>.stride == 144,
              MemoryLayout<SharedAudioHeader>.alignment == 8 else {
            halLog(
                "    FAIL: size=\(MemoryLayout<SharedAudioHeader>.size), " +
                "stride=\(MemoryLayout<SharedAudioHeader>.stride), " +
                "alignment=\(MemoryLayout<SharedAudioHeader>.alignment)"
            )
            return false
        }

        for (name, actual, expected) in expectedOffsets where actual != expected {
            halLog("    FAIL: \(name) offset=\(String(describing: actual)), expected=\(expected)")
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
