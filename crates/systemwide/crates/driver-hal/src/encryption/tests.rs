use super::audio_cipher::AudioCipher;
use super::encrypted::encrypted_byte_size;
use super::encrypted::encrypted_to_samples;
use super::encrypted::encrypted_to_samples_into;
use super::misc::AUTH_TAG_SIZE;
use super::misc::compute_fingerprint;
use super::misc::session_key_path_from_env;
use super::samples::samples_to_encrypted;
use super::samples::samples_to_encrypted_into;

fn generate_test_key() -> [u8; 32] {
    [0x42; 32]
}

#[test]
fn test_session_key_path_prefers_explicit_and_lab_overrides() {
    use std::ffi::OsString;
    use std::path::PathBuf;

    assert_eq!(
        session_key_path_from_env(
            Some(OsString::from("/tmp/explicit-session.key")),
            Some(OsString::from("/tmp/ignored-runtime")),
            Some(OsString::from("/Users/ignored")),
            501,
            true,
        ),
        PathBuf::from("/tmp/explicit-session.key")
    );
    assert_eq!(
        session_key_path_from_env(
            None,
            Some(OsString::from("/tmp/systemwide-lab")),
            Some(OsString::from("/Users/ignored")),
            501,
            true,
        ),
        PathBuf::from("/tmp/systemwide-lab/session.key")
    );
}

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let key = generate_test_key();
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
    let key = generate_test_key();
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
    let key = generate_test_key();
    let cipher = AudioCipher::new(&key);

    let samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
    let ciphertext = cipher.encrypt(&samples, 1);

    // Decrypt with wrong frame counter should fail
    assert!(cipher.decrypt(&ciphertext, 2).is_none());
}

#[test]
fn test_different_keys() {
    let key1 = generate_test_key();
    let key2 = [0x24; 32];
    let cipher1 = AudioCipher::new(&key1);
    let cipher2 = AudioCipher::new(&key2);

    let samples: Vec<f32> = vec![0.5, -0.5, 0.25, -0.25];
    let ciphertext = cipher1.encrypt(&samples, 1);

    // Decryption with different key should fail
    assert!(cipher2.decrypt(&ciphertext, 1).is_none());
}

#[test]
fn test_fingerprint_consistency() {
    let key = generate_test_key();
    let cipher = AudioCipher::new(&key);
    let computed = compute_fingerprint(&key);

    assert_eq!(cipher.fingerprint(), &computed);
}

#[test]
fn test_special_float_values() {
    let key = generate_test_key();
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
    let key = generate_test_key();
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
    let key = generate_test_key();
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
    let key = generate_test_key();
    let cipher = AudioCipher::new(&key);
    let samples = vec![0.5f32; 100];

    let mut too_small = vec![0u8; 10];
    assert!(cipher.encrypt_into(&samples, 1, &mut too_small).is_none());
}

#[test]
fn test_decrypt_into_buffer_too_small() {
    let key = generate_test_key();
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
