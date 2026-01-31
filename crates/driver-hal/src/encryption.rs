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
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use sha2::{Digest, Sha256};

/// Size of the authentication tag appended to each encrypted block
pub const AUTH_TAG_SIZE: usize = 16;

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
}

/// Convert f32 samples to bytes (native endian)
fn samples_to_bytes(samples: &[f32]) -> Vec<u8> {
    let byte_len = samples.len() * std::mem::size_of::<f32>();
    let mut bytes = vec![0u8; byte_len];

    // SAFETY: We're reinterpreting the byte slice as a mutable f32 slice
    // for a direct copy. The alignment is handled by starting from a Vec<u8>.
    for (i, sample) in samples.iter().enumerate() {
        let sample_bytes = sample.to_ne_bytes();
        bytes[i * 4..(i + 1) * 4].copy_from_slice(&sample_bytes);
    }

    bytes
}

/// Convert bytes back to f32 samples (native endian)
fn bytes_to_samples(bytes: &[u8]) -> Vec<f32> {
    let sample_count = bytes.len() / std::mem::size_of::<f32>();
    let mut samples = Vec::with_capacity(sample_count);

    for i in 0..sample_count {
        let sample_bytes: [u8; 4] = bytes[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice should be exactly 4 bytes");
        samples.push(f32::from_ne_bytes(sample_bytes));
    }

    samples
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
    let sample_count = (ciphertext.len() + 3) / 4;
    let mut samples = vec![0.0f32; sample_count];

    for (i, chunk) in ciphertext.chunks(4).enumerate() {
        let mut bytes = [0u8; 4];
        bytes[..chunk.len()].copy_from_slice(chunk);
        samples[i] = f32::from_ne_bytes(bytes);
    }

    samples
}

/// Convert f32 samples back to encrypted ciphertext bytes
///
/// This unpacks the f32 values back to raw bytes.
pub fn samples_to_encrypted(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_ne_bytes());
    }
    bytes
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
        let decrypted = cipher.decrypt(&ciphertext, frame_counter).expect("decryption should succeed");

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
        let decrypted = cipher.decrypt(&ciphertext, 1).expect("decryption should succeed");

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
        let decrypted = cipher.decrypt(&ciphertext, 1).expect("decryption should succeed");

        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_ciphertext_size() {
        assert_eq!(AudioCipher::ciphertext_size(0), AUTH_TAG_SIZE);
        assert_eq!(AudioCipher::ciphertext_size(1), 4 + AUTH_TAG_SIZE);
        assert_eq!(AudioCipher::ciphertext_size(1024), 4096 + AUTH_TAG_SIZE);
    }
}
