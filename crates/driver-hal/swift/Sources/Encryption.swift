// Encryption.swift - ChaCha20-Poly1305 encryption for audio data
//
// Provides encryption for audio data exchanged between the Swift HAL driver
// and Rust daemon via shared memory. Uses CryptoKit for native performance.
//
// Security Model:
// - ChaCha20-Poly1305 AEAD cipher
// - Key is loaded from ~/.config/sotf/session.key
// - Frame counter provides unique nonces
// - Poly1305 authentication detects tampering
//
// Must match the Rust implementation in driver-hal/src/encryption.rs

import Foundation
import CryptoKit
import CommonCrypto

/// Size of the authentication tag (16 bytes for Poly1305)
let kAuthTagSize = 16

/// Audio encryption cipher using ChaCha20-Poly1305 (via CryptoKit)
final class AudioCipher {
    private let key: SymmetricKey
    private let fingerprint: [UInt8]

    /// Create a new AudioCipher from a 256-bit key
    /// - Parameter keyBytes: 32-byte (256-bit) encryption key
    init(keyBytes: [UInt8]) {
        precondition(keyBytes.count == 32, "Key must be exactly 32 bytes")
        self.key = SymmetricKey(data: keyBytes)

        // Compute fingerprint as first 8 bytes of SHA256(key)
        var hash = [UInt8](repeating: 0, count: Int(CC_SHA256_DIGEST_LENGTH))
        keyBytes.withUnsafeBytes { keyPtr in
            _ = CC_SHA256(keyPtr.baseAddress, CC_LONG(keyBytes.count), &hash)
        }
        self.fingerprint = Array(hash[0..<8])
    }

    /// Get the key fingerprint (first 8 bytes of SHA256 of key)
    func getFingerprint() -> [UInt8] {
        return fingerprint
    }

    /// Get the fingerprint as a hex string
    func getFingerprintHex() -> String {
        return fingerprint.map { String(format: "%02x", $0) }.joined()
    }

    /// Encrypt audio samples
    /// - Parameters:
    ///   - samples: Audio samples as Float array
    ///   - frameCounter: Unique counter for nonce generation
    /// - Returns: Encrypted ciphertext with authentication tag
    func encrypt(samples: [Float], frameCounter: UInt64) -> [UInt8]? {
        // Convert samples to bytes (native endian)
        let plaintext = samplesToBytes(samples)

        // Create nonce from frame counter (12 bytes)
        // Format: [4 bytes zero padding] [8 bytes frame_counter as big-endian]
        var nonceBytes = [UInt8](repeating: 0, count: 12)
        withUnsafeBytes(of: frameCounter.bigEndian) { fcBytes in
            for (i, byte) in fcBytes.enumerated() {
                nonceBytes[4 + i] = byte
            }
        }

        do {
            let nonce = try ChaChaPoly.Nonce(data: nonceBytes)
            let sealedBox = try ChaChaPoly.seal(plaintext, using: key, nonce: nonce)
            // CryptoKit combines ciphertext and tag - return as single array
            return Array(sealedBox.ciphertext) + Array(sealedBox.tag)
        } catch {
            halLog("Encryption failed: \(error)")
            return nil
        }
    }

    /// Encrypt samples in-place to a buffer
    /// - Parameters:
    ///   - samples: Pointer to audio samples
    ///   - sampleCount: Number of samples
    ///   - frameCounter: Unique counter for nonce generation
    ///   - output: Output buffer (must be at least sampleCount * 4 + 16 bytes)
    /// - Returns: Number of bytes written, or 0 on failure
    func encryptToBuffer(_ samples: UnsafePointer<Float>, sampleCount: Int, frameCounter: UInt64, output: UnsafeMutablePointer<UInt8>) -> Int {
        // Convert samples to bytes
        let byteCount = sampleCount * MemoryLayout<Float>.size
        var plaintext = [UInt8](repeating: 0, count: byteCount)
        memcpy(&plaintext, samples, byteCount)

        // Create nonce
        var nonceBytes = [UInt8](repeating: 0, count: 12)
        withUnsafeBytes(of: frameCounter.bigEndian) { fcBytes in
            for (i, byte) in fcBytes.enumerated() {
                nonceBytes[4 + i] = byte
            }
        }

        do {
            let nonce = try ChaChaPoly.Nonce(data: nonceBytes)
            let sealedBox = try ChaChaPoly.seal(plaintext, using: key, nonce: nonce)

            // Copy ciphertext and tag to output
            let ciphertext = Array(sealedBox.ciphertext)
            let tag = Array(sealedBox.tag)
            memcpy(output, ciphertext, ciphertext.count)
            memcpy(output.advanced(by: ciphertext.count), tag, tag.count)

            return ciphertext.count + tag.count
        } catch {
            halLog("Encryption failed: \(error)")
            return 0
        }
    }

    /// Decrypt audio samples
    /// - Parameters:
    ///   - ciphertext: Encrypted data with authentication tag
    ///   - frameCounter: Same counter used during encryption
    /// - Returns: Decrypted samples if authentication succeeds, nil if tampered
    func decrypt(ciphertext: [UInt8], frameCounter: UInt64) -> [Float]? {
        guard ciphertext.count >= kAuthTagSize else {
            halLog("Ciphertext too short")
            return nil
        }

        // Create nonce from frame counter
        var nonceBytes = [UInt8](repeating: 0, count: 12)
        withUnsafeBytes(of: frameCounter.bigEndian) { fcBytes in
            for (i, byte) in fcBytes.enumerated() {
                nonceBytes[4 + i] = byte
            }
        }

        do {
            let nonce = try ChaChaPoly.Nonce(data: nonceBytes)

            // Split ciphertext and tag
            let ctLen = ciphertext.count - kAuthTagSize
            let ct = Array(ciphertext[0..<ctLen])
            let tag = Array(ciphertext[ctLen...])

            let sealedBox = try ChaChaPoly.SealedBox(nonce: nonce, ciphertext: ct, tag: tag)
            let plaintext = try ChaChaPoly.open(sealedBox, using: key)

            // Convert bytes back to samples
            return bytesToSamples(Array(plaintext))
        } catch {
            halLog("Decryption failed: \(error)")
            return nil
        }
    }

    /// Decrypt from buffer in-place
    /// - Parameters:
    ///   - ciphertext: Pointer to encrypted data with tag
    ///   - ciphertextLen: Length of ciphertext including tag
    ///   - frameCounter: Same counter used during encryption
    ///   - output: Output buffer for decrypted samples
    /// - Returns: Number of samples written, or 0 on failure
    func decryptFromBuffer(_ ciphertext: UnsafePointer<UInt8>, ciphertextLen: Int, frameCounter: UInt64, output: UnsafeMutablePointer<Float>) -> Int {
        guard ciphertextLen >= kAuthTagSize else { return 0 }

        // Create nonce
        var nonceBytes = [UInt8](repeating: 0, count: 12)
        withUnsafeBytes(of: frameCounter.bigEndian) { fcBytes in
            for (i, byte) in fcBytes.enumerated() {
                nonceBytes[4 + i] = byte
            }
        }

        do {
            let nonce = try ChaChaPoly.Nonce(data: nonceBytes)

            // Split ciphertext and tag
            let ctLen = ciphertextLen - kAuthTagSize
            let ct = Array(UnsafeBufferPointer(start: ciphertext, count: ctLen))
            let tag = Array(UnsafeBufferPointer(start: ciphertext.advanced(by: ctLen), count: kAuthTagSize))

            let sealedBox = try ChaChaPoly.SealedBox(nonce: nonce, ciphertext: ct, tag: tag)
            let plaintext = try ChaChaPoly.open(sealedBox, using: key)

            // Copy samples to output
            let sampleCount = plaintext.count / MemoryLayout<Float>.size
            plaintext.withUnsafeBytes { ptBytes in
                memcpy(output, ptBytes.baseAddress, plaintext.count)
            }

            return sampleCount
        } catch {
            halLog("Decryption failed: \(error)")
            return 0
        }
    }

    /// Convert Float samples to bytes (native endian)
    private func samplesToBytes(_ samples: [Float]) -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: samples.count * MemoryLayout<Float>.size)
        for (i, sample) in samples.enumerated() {
            withUnsafeBytes(of: sample) { sampleBytes in
                for (j, byte) in sampleBytes.enumerated() {
                    bytes[i * 4 + j] = byte
                }
            }
        }
        return bytes
    }

    /// Convert bytes to Float samples (native endian)
    private func bytesToSamples(_ bytes: [UInt8]) -> [Float] {
        let sampleCount = bytes.count / MemoryLayout<Float>.size
        var samples = [Float](repeating: 0, count: sampleCount)
        for i in 0..<sampleCount {
            var value: Float = 0
            withUnsafeMutableBytes(of: &value) { valueBytes in
                for j in 0..<4 {
                    valueBytes[j] = bytes[i * 4 + j]
                }
            }
            samples[i] = value
        }
        return samples
    }
}

/// Encryption key manager
final class EncryptionKeyManager {
    private var cipher: AudioCipher?
    private var keyPath: String
    private var lastMtime: Date?
    private var enabled: Bool = false

    /// Shared instance
    static let shared = EncryptionKeyManager()

    private init() {
        let home = NSHomeDirectory()
        keyPath = "\(home)/.config/sotf/session.key"
        loadKey()
    }

    /// Load the encryption key from file
    private func loadKey() {
        guard FileManager.default.fileExists(atPath: keyPath) else {
            halLog("Key file not found at \(keyPath)")
            cipher = nil
            return
        }

        do {
            // Check file permissions - should be 0600 or 0640 (owner read/write only)
            let attrs = try FileManager.default.attributesOfItem(atPath: keyPath)
            if let permissions = attrs[.posixPermissions] as? Int {
                // Allow 0600 (owner rw) or 0640 (owner rw, group r)
                let mode = permissions & 0o777
                if mode != 0o600 && mode != 0o640 {
                    halLog("Warning: Key file has insecure permissions \(String(mode, radix: 8)) (expected 600 or 640)")
                    // Continue loading but warn - daemon may have created with different permissions
                }
            }

            let keyData = try Data(contentsOf: URL(fileURLWithPath: keyPath))
            guard keyData.count == 32 else {
                halLog("Invalid key file size: \(keyData.count) bytes (expected 32)")
                cipher = nil
                return
            }

            let keyBytes = [UInt8](keyData)
            cipher = AudioCipher(keyBytes: keyBytes)
            lastMtime = attrs[.modificationDate] as? Date
            halLog("Loaded encryption key, fingerprint: \(cipher?.getFingerprintHex() ?? "nil")")
        } catch {
            halLog("Failed to load encryption key: \(error)")
            cipher = nil
        }
    }

    /// Check if the key file has changed and reload if necessary
    func checkAndReload() -> Bool {
        guard FileManager.default.fileExists(atPath: keyPath) else {
            if cipher != nil {
                halLog("Key file deleted")
                cipher = nil
                return true
            }
            return false
        }

        if let attrs = try? FileManager.default.attributesOfItem(atPath: keyPath),
           let mtime = attrs[.modificationDate] as? Date {
            if lastMtime == nil || mtime != lastMtime {
                loadKey()
                return true
            }
        }

        return false
    }

    /// Get the current cipher (may be nil if key not loaded)
    func getCipher() -> AudioCipher? {
        return cipher
    }

    /// Get the key fingerprint
    func getFingerprint() -> [UInt8]? {
        return cipher?.getFingerprint()
    }

    /// Check if encryption is enabled AND cipher is available
    ///
    /// Returns true only if both `enabled` flag is set AND a valid cipher
    /// has been loaded. This prevents the case where encryption is "enabled"
    /// but no key is available.
    var isEnabled: Bool {
        get { enabled && cipher != nil }
        set {
            enabled = newValue
            if newValue && cipher == nil {
                halLog("Warning: Encryption enabled but no cipher available (key may not be loaded)")
            } else {
                halLog("Encryption \(enabled ? "enabled" : "disabled")")
            }
        }
    }

    /// Check if encryption is available (cipher loaded, regardless of enabled state)
    var isAvailable: Bool {
        return cipher != nil
    }
}
