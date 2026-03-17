//! Audio encryption module for secure shared memory communication
//!
//! Provides ChaCha20-Poly1305 AEAD encryption for audio data exchanged
//! between the Swift HAL driver and Rust daemon via shared memory.
//!
//! # Security Model
//!
//! - Each session generates a new 256-bit encryption key
//! - Key is stored in `~/.config/sotf/session.key` with mode 0640
//! - Key fingerprint (first 8 bytes of SHA256) is stored in shared memory header
//! - Frame counter provides unique nonces (never reused)
//! - Poly1305 authentication tag detects tampering
//!
//! # Performance
//!
//! ChaCha20-Poly1305 is chosen for:
//! - No hardware acceleration required (works on all CPUs)
//! - Excellent performance for audio-sized blocks
//! - Resistance to timing attacks

use chacha20poly1305::{
    ChaCha20Poly1305, Nonce, Tag,
    aead::{Aead, AeadInPlace, KeyInit},
};
use sha2::{Digest, Sha256};

/// Size of the authentication tag appended to each encrypted block
pub const AUTH_TAG_SIZE: usize = 16;

/// Required byte buffer size for encrypting N samples (samples * 4 + auth tag)
pub const fn encrypted_byte_size(sample_count: usize) -> usize {
    sample_count * 4 + AUTH_TAG_SIZE
}

/// Required f32 buffer size for storing encrypted data (ceiling division)
pub const fn encrypted_sample_slots(sample_count: usize) -> usize {
    encrypted_byte_size(sample_count).div_ceil(4)
}

/// Audio encryption cipher using ChaCha20-Poly1305
pub struct AudioCipher {
    cipher: ChaCha20Poly1305,
    fingerprint: [u8; 8],
}

impl AudioCipher {
    /// Create a new AudioCipher from a 256-bit key
    ///
    /// # Arguments
    /// * `key` - 32-byte (256-bit) encryption key
    ///
    /// # Returns
    /// A new AudioCipher instance with the computed key fingerprint
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .expect("32-byte key is always valid for ChaCha20Poly1305");

        // Compute fingerprint as first 8 bytes of SHA256(key)
        let mut hasher = Sha256::new();
        hasher.update(key);
        let hash = hasher.finalize();
        let mut fingerprint = [0u8; 8];
        fingerprint.copy_from_slice(&hash[..8]);

        Self {
            cipher,
            fingerprint,
        }
    }

    /// Get the key fingerprint (first 8 bytes of SHA256 of key)
    pub fn fingerprint(&self) -> &[u8; 8] {
        &self.fingerprint
    }

    /// Encrypt audio samples
    ///
    /// # Arguments
    /// * `samples` - Audio samples as f32 slice
    /// * `frame_counter` - Unique counter for nonce generation (MUST be unique per encryption)
    ///
    /// # Returns
    /// Encrypted ciphertext with authentication tag appended (16 bytes longer than input)
    ///
    /// # Panics
    /// Panics if frame_counter is reused (nonce reuse is catastrophic for security)
    pub fn encrypt(&self, samples: &[f32], frame_counter: u64) -> Vec<u8> {
        // Convert samples to bytes
        let plaintext = samples_to_bytes(samples);

        // Create nonce from frame counter (12 bytes)
        // Format: [4 bytes zero padding] [8 bytes frame_counter as big-endian]
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&frame_counter.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with authentication
        self.cipher
            .encrypt(nonce, plaintext.as_ref())
            .expect("encryption should not fail")
    }

    /// Decrypt audio samples
    ///
    /// # Arguments
    /// * `ciphertext` - Encrypted data with authentication tag
    /// * `frame_counter` - Same counter used during encryption
    ///
    /// # Returns
    /// Decrypted samples if authentication succeeds, None if tampered
    pub fn decrypt(&self, ciphertext: &[u8], frame_counter: u64) -> Option<Vec<f32>> {
        // Create nonce from frame counter
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&frame_counter.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decrypt and verify authentication
        let plaintext = self.cipher.decrypt(nonce, ciphertext).ok()?;

        // Convert bytes back to samples
        Some(bytes_to_samples(&plaintext))
    }

    /// Calculate the ciphertext size for a given number of samples
    pub fn ciphertext_size(sample_count: usize) -> usize {
        sample_count * std::mem::size_of::<f32>() + AUTH_TAG_SIZE
    }

    /// Encrypt audio samples into a pre-allocated byte buffer (allocation-free hot path)
    ///
    /// # Arguments
    /// * `samples` - Audio samples as f32 slice
    /// * `frame_counter` - Unique counter for nonce generation
    /// * `output` - Pre-allocated buffer, must be at least `encrypted_byte_size(samples.len())` bytes
    ///
    /// # Returns
    /// Number of bytes written to output, or None if output buffer too small
    pub fn encrypt_into(
        &self,
        samples: &[f32],
        frame_counter: u64,
        output: &mut [u8],
    ) -> Option<usize> {
        let required_size = encrypted_byte_size(samples.len());
        if output.len() < required_size {
            return None;
        }

        // Copy samples as bytes directly into output buffer
        let sample_bytes = samples.len() * 4;
        samples_to_bytes_into(samples, &mut output[..sample_bytes]);

        // Create nonce from frame counter
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&frame_counter.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt in place and get auth tag
        let tag = self
            .cipher
            .encrypt_in_place_detached(nonce, &[], &mut output[..sample_bytes])
            .expect("encryption should not fail");

        // Append auth tag
        output[sample_bytes..sample_bytes + AUTH_TAG_SIZE].copy_from_slice(&tag);

        Some(required_size)
    }

    /// Decrypt ciphertext into a pre-allocated f32 buffer (allocation-free hot path)
    ///
    /// # Arguments
    /// * `ciphertext` - Encrypted data with authentication tag
    /// * `frame_counter` - Same counter used during encryption
    /// * `output` - Pre-allocated buffer for decrypted samples
    ///
    /// # Returns
    /// Number of samples written, or None if decryption failed or buffer too small
    pub fn decrypt_into(
        &self,
        ciphertext: &[u8],
        frame_counter: u64,
        output: &mut [f32],
    ) -> Option<usize> {
        if ciphertext.len() < AUTH_TAG_SIZE {
            return None;
        }

        let sample_bytes = ciphertext.len() - AUTH_TAG_SIZE;
        let sample_count = sample_bytes / 4;

        if output.len() < sample_count {
            return None;
        }

        // Create nonce from frame counter
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&frame_counter.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Extract auth tag
        let tag = Tag::from_slice(&ciphertext[sample_bytes..]);

        // Copy ciphertext (without tag) to output buffer as bytes, then decrypt in place
        // We need a mutable byte view of the output f32 slice
        let output_bytes = samples_as_bytes_mut(&mut output[..sample_count]);
        output_bytes.copy_from_slice(&ciphertext[..sample_bytes]);

        // Decrypt in place
        self.cipher
            .decrypt_in_place_detached(nonce, &[], output_bytes, tag)
            .ok()?;

        Some(sample_count)
    }
}

/// Convert f32 samples to bytes (native endian) - allocating version
fn samples_to_bytes(samples: &[f32]) -> Vec<u8> {
    let byte_len = std::mem::size_of_val(samples);
    let mut bytes = vec![0u8; byte_len];
    samples_to_bytes_into(samples, &mut bytes);
    bytes
}

/// Convert f32 samples to bytes into a pre-allocated buffer (allocation-free)
///
/// # Panics
/// Panics if output buffer is smaller than samples.len() * 4
fn samples_to_bytes_into(samples: &[f32], output: &mut [u8]) {
    debug_assert!(output.len() >= samples.len() * 4);
    for (i, sample) in samples.iter().enumerate() {
        output[i * 4..(i + 1) * 4].copy_from_slice(&sample.to_le_bytes());
    }
}

/// Get a mutable byte view of f32 samples (zero-copy)
///
/// # Safety
/// This reinterprets f32 memory as bytes. The resulting bytes are in native endian.
fn samples_as_bytes_mut(samples: &mut [f32]) -> &mut [u8] {
    // SAFETY: f32 can always be safely reinterpreted as bytes
    // The alignment of f32 (4) is >= alignment of u8 (1)
    let ptr = samples.as_mut_ptr() as *mut u8;
    let len = samples.len() * 4;
    // SAFETY: The resulting slice covers the same memory as the input slice
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}

/// Get an immutable byte view of f32 samples (zero-copy)
#[allow(dead_code)]
fn samples_as_bytes(samples: &[f32]) -> &[u8] {
    // SAFETY: f32 can always be safely reinterpreted as bytes
    let ptr = samples.as_ptr() as *const u8;
    let len = samples.len() * 4;
    // SAFETY: The resulting slice covers the same memory as the input slice
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// Convert bytes back to f32 samples (native endian) - allocating version
fn bytes_to_samples(bytes: &[u8]) -> Vec<f32> {
    let sample_count = bytes.len() / std::mem::size_of::<f32>();
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let sample_bytes: [u8; 4] = bytes[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice should be exactly 4 bytes");
        samples.push(f32::from_le_bytes(sample_bytes));
    }

    samples
}

/// Convert bytes to f32 samples into a pre-allocated buffer (allocation-free)
///
/// # Returns
/// Number of samples written
#[allow(dead_code)]
fn bytes_to_samples_into(bytes: &[u8], output: &mut [f32]) -> usize {
    let sample_count = bytes.len() / 4;
    let to_write = sample_count.min(output.len());

    for i in 0..to_write {
        let sample_bytes: [u8; 4] = bytes[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice should be exactly 4 bytes");
        output[i] = f32::from_le_bytes(sample_bytes);
    }

    to_write
}

/// Generate a new random 256-bit encryption key
pub fn generate_key() -> [u8; 32] {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Compute the fingerprint (first 8 bytes of SHA256) for a key
pub fn compute_fingerprint(key: &[u8; 32]) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    let hash = hasher.finalize();
    let mut fingerprint = [0u8; 8];
    fingerprint.copy_from_slice(&hash[..8]);
    fingerprint
}

/// Format a fingerprint as a hex string
pub fn fingerprint_to_hex(fingerprint: &[u8; 8]) -> String {
    hex::encode(fingerprint)
}

/// Convert encrypted ciphertext bytes to f32 samples for storage in ring buffer
///
/// This packs the raw bytes into f32 values using native byte order.
/// The resulting samples are NOT audio data - they're encrypted bytes stored as f32.
pub fn encrypted_to_samples(ciphertext: &[u8]) -> Vec<f32> {
    // Round up to ensure we have space for all bytes
    let sample_count = ciphertext.len().div_ceil(4);
    let mut samples = vec![0.0f32; sample_count];
    encrypted_to_samples_into(ciphertext, &mut samples);
    samples
}

/// Convert encrypted ciphertext bytes to f32 samples into a pre-allocated buffer (allocation-free)
///
/// # Returns
/// Number of f32 slots written
pub fn encrypted_to_samples_into(ciphertext: &[u8], output: &mut [f32]) -> usize {
    let sample_count = ciphertext.len().div_ceil(4);
    let to_write = sample_count.min(output.len());

    for (i, chunk) in ciphertext.chunks(4).take(to_write).enumerate() {
        let mut bytes = [0u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        output[i] = f32::from_le_bytes(bytes);
    }

    to_write
}

/// Convert f32 samples back to encrypted ciphertext bytes
///
/// This unpacks the f32 values back to raw bytes.
pub fn samples_to_encrypted(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Convert f32 samples to encrypted ciphertext bytes into a pre-allocated buffer (allocation-free)
///
/// # Returns
/// Number of bytes written (always samples.len() * 4)
pub fn samples_to_encrypted_into(samples: &[f32], output: &mut [u8]) -> usize {
    let byte_count = samples.len() * 4;
    debug_assert!(output.len() >= byte_count);

    for (i, sample) in samples.iter().enumerate() {
        output[i * 4..(i + 1) * 4].copy_from_slice(&sample.to_le_bytes());
    }

    byte_count
}

/// Get the path to the session encryption key (~/.config/sotf/session.key)
pub fn get_session_key_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home).join(".config/sotf/session.key")
}

/// Load the session encryption key from disk
pub fn load_session_key() -> std::io::Result<[u8; 32]> {
    use std::io::Read;
    let path = get_session_key_path();
    let mut file = std::fs::File::open(path)?;
    let mut key = [0u8; 32];
    file.read_exact(&mut key)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        // Test with various audio patterns
        let samples: Vec<f32> = (0..1024)
            .map(|i| (i as f32 / 1024.0 * std::f32::consts::PI * 2.0).sin())
            .collect();

        let frame_counter = 1;
        let ciphertext = cipher.encrypt(&samples, frame_counter);

        // Verify ciphertext is larger (has auth tag)
        assert_eq!(ciphertext.len(), samples.len() * 4 + AUTH_TAG_SIZE);

        // Decrypt and verify
        let decrypted = cipher
            .decrypt(&ciphertext, frame_counter)
            .expect("decryption should succeed");

        // Verify bit-for-bit accuracy
        assert_eq!(samples.len(), decrypted.len());
        for (orig, dec) in samples.iter().zip(decrypted.iter()) {
            assert_eq!(orig.to_bits(), dec.to_bits(), "Sample mismatch");
        }
    }

    #[test]
    fn test_tamper_detection() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        let samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
        let mut ciphertext = cipher.encrypt(&samples, 1);

        // Tamper with the ciphertext
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }

        // Decryption should fail
        assert!(cipher.decrypt(&ciphertext, 1).is_none());
    }

    #[test]
    fn test_wrong_frame_counter() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        let samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
        let ciphertext = cipher.encrypt(&samples, 1);

        // Decrypt with wrong frame counter should fail
        assert!(cipher.decrypt(&ciphertext, 2).is_none());
    }

    #[test]
    fn test_different_keys() {
        let key1 = generate_key();
        let key2 = generate_key();
        let cipher1 = AudioCipher::new(&key1);
        let cipher2 = AudioCipher::new(&key2);

        let samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
        let ciphertext = cipher1.encrypt(&samples, 1);

        // Decryption with different key should fail
        assert!(cipher2.decrypt(&ciphertext, 1).is_none());
    }

    #[test]
    fn test_fingerprint_consistency() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);
        let computed = compute_fingerprint(&key);

        assert_eq!(cipher.fingerprint(), &computed);
    }

    #[test]
    fn test_special_float_values() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        // Test special float values
        let samples = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            f32::MIN_POSITIVE,
            f32::EPSILON,
            0.999999,
        ];

        let ciphertext = cipher.encrypt(&samples, 1);
        let decrypted = cipher
            .decrypt(&ciphertext, 1)
            .expect("decryption should succeed");

        for (orig, dec) in samples.iter().zip(decrypted.iter()) {
            assert_eq!(orig.to_bits(), dec.to_bits());
        }
    }

    #[test]
    fn test_empty_samples() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        let samples: Vec<f32> = vec![];
        let ciphertext = cipher.encrypt(&samples, 1);
        let decrypted = cipher
            .decrypt(&ciphertext, 1)
            .expect("decryption should succeed");

        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_ciphertext_size() {
        assert_eq!(AudioCipher::ciphertext_size(0), AUTH_TAG_SIZE);
        assert_eq!(AudioCipher::ciphertext_size(1), 4 + AUTH_TAG_SIZE);
        assert_eq!(AudioCipher::ciphertext_size(1024), 4096 + AUTH_TAG_SIZE);
    }

    #[test]
    fn test_encrypt_into_decrypt_into_roundtrip() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);

        // Test with various audio patterns
        let samples: Vec<f32> = (0..1024)
            .map(|i| (i as f32 / 1024.0 * std::f32::consts::PI * 2.0).sin())
            .collect();

        let frame_counter = 42;

        // Encrypt into pre-allocated buffer
        let mut ciphertext = vec![0u8; encrypted_byte_size(samples.len())];
        let encrypted_len = cipher
            .encrypt_into(&samples, frame_counter, &mut ciphertext)
            .expect("encryption should succeed");
        assert_eq!(encrypted_len, ciphertext.len());

        // Decrypt into pre-allocated buffer
        let mut decrypted = vec![0.0f32; samples.len()];
        let sample_count = cipher
            .decrypt_into(&ciphertext, frame_counter, &mut decrypted)
            .expect("decryption should succeed");
        assert_eq!(sample_count, samples.len());

        // Verify bit-for-bit accuracy
        for (orig, dec) in samples.iter().zip(decrypted.iter()) {
            assert_eq!(orig.to_bits(), dec.to_bits(), "Sample mismatch");
        }
    }

    #[test]
    fn test_encrypt_into_buffer_too_small() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);
        let samples = vec![0.5f32; 100];

        let mut too_small = vec![0u8; 10];
        assert!(cipher.encrypt_into(&samples, 1, &mut too_small).is_none());
    }

    #[test]
    fn test_decrypt_into_buffer_too_small() {
        let key = generate_key();
        let cipher = AudioCipher::new(&key);
        let samples = vec![0.5f32; 100];

        let ciphertext = cipher.encrypt(&samples, 1);

        let mut too_small = vec![0.0f32; 10];
        assert!(
            cipher
                .decrypt_into(&ciphertext, 1, &mut too_small)
                .is_none()
        );
    }

    #[test]
    fn test_encrypted_to_samples_into() {
        let ciphertext = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut output = vec![0.0f32; 10];

        let written = encrypted_to_samples_into(&ciphertext, &mut output);
        assert_eq!(written, 3); // 9 bytes = 3 f32 slots (rounded up)

        // Compare with allocating version
        let expected = encrypted_to_samples(&ciphertext);
        for i in 0..written {
            assert_eq!(output[i].to_bits(), expected[i].to_bits());
        }
    }

    #[test]
    fn test_samples_to_encrypted_into() {
        let samples = vec![1.0f32, 2.0, 3.0];
        let mut output = vec![0u8; 20];

        let written = samples_to_encrypted_into(&samples, &mut output);
        assert_eq!(written, 12); // 3 f32 = 12 bytes

        // Compare with allocating version
        let expected = samples_to_encrypted(&samples);
        assert_eq!(&output[..written], &expected[..]);
    }
}
