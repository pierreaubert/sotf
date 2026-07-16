/// Convert f32 samples to bytes (native endian) - allocating version
pub(super) fn samples_to_bytes(samples: &[f32]) -> Vec<u8> {
    let byte_len = std::mem::size_of_val(samples);
    let mut bytes = vec![0u8; byte_len];
    samples_to_bytes_into(samples, &mut bytes);
    bytes
}

/// Convert f32 samples to bytes into a pre-allocated buffer (allocation-free)
///
/// # Panics
/// Panics if output buffer is smaller than samples.len() * 4
pub(super) fn samples_to_bytes_into(samples: &[f32], output: &mut [u8]) {
    debug_assert!(output.len() >= samples.len() * 4);
    for (i, sample) in samples.iter().enumerate() {
        output[i * 4..(i + 1) * 4].copy_from_slice(&sample.to_le_bytes());
    }
}

/// Get a mutable byte view of f32 samples (zero-copy).
pub(super) fn samples_as_bytes_mut(samples: &mut [f32]) -> &mut [u8] {
    bytemuck::cast_slice_mut(samples)
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
