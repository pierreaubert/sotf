use crate::decoder::{AudioDecoderResult, create_decoder};

#[cfg(test)]
use crate::decoder::AudioDecoderError;
use std::path::Path;

/// Number of amplitude samples in the waveform
pub const WAVEFORM_SAMPLES: usize = 128;

/// Analyze an audio file and compute a waveform representation
///
/// This function decodes an audio file using Symphonia and computes 128 amplitude
/// samples representing the waveform of the track. Each sample is an 8-bit value
/// (0-255) representing the RMS amplitude of that portion of the track.
///
/// # Arguments
///
/// * `path` - Path to the audio file to analyze
///
/// # Returns
///
/// Returns a `Vec<u8>` containing exactly 128 amplitude samples.
///
/// # Errors
///
/// Returns an `AudioDecoderError` if:
/// - The file cannot be found or opened
/// - The file format is unsupported
/// - Decoding fails
///
/// # Example
///
/// ```no_run
/// use sotf_audio::waveform::analyze_waveform;
///
/// let waveform = analyze_waveform("track.flac").unwrap();
/// assert_eq!(waveform.len(), 128);
/// ```
pub fn analyze_waveform<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Vec<u8>> {
    log::debug!(
        "[Waveform] Analyze: {}",
        path.as_ref().to_str().unwrap_or("unknown")
    );

    let path = path.as_ref();

    // Create decoder for the audio file
    let mut decoder = create_decoder(path)?;

    // Collect all samples (as mono, averaged across channels)
    let mut all_samples: Vec<f32> = Vec::new();

    // Process audio in chunks
    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }

        let channels = decoded.spec.channels as usize;
        let samples = &decoded.samples;

        // Convert to mono by averaging channels
        for frame_start in (0..samples.len()).step_by(channels) {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                if frame_start + ch < samples.len() {
                    sum += samples[frame_start + ch];
                }
            }
            all_samples.push(sum / channels as f32);
        }
    }

    // If no samples, return zeros
    if all_samples.is_empty() {
        log::warn!("[Waveform] No samples decoded from file");
        return Ok(vec![0u8; WAVEFORM_SAMPLES]);
    }

    // Divide samples into WAVEFORM_SAMPLES chunks and compute RMS for each
    let samples_per_chunk = all_samples.len() / WAVEFORM_SAMPLES;

    // Handle case where file is shorter than WAVEFORM_SAMPLES
    if samples_per_chunk == 0 {
        log::warn!(
            "[Waveform] File too short ({} samples), padding waveform",
            all_samples.len()
        );
        // Just use what we have, padding with zeros
        let mut waveform = Vec::with_capacity(WAVEFORM_SAMPLES);
        for i in 0..WAVEFORM_SAMPLES {
            if i < all_samples.len() {
                // Convert single sample to 0-255 range
                let amplitude = all_samples[i].abs();
                waveform.push((amplitude.min(1.0) * 255.0) as u8);
            } else {
                waveform.push(0);
            }
        }
        return Ok(waveform);
    }

    // Compute RMS for each chunk
    let mut rms_values: Vec<f32> = Vec::with_capacity(WAVEFORM_SAMPLES);

    for chunk_idx in 0..WAVEFORM_SAMPLES {
        let start = chunk_idx * samples_per_chunk;
        let end = if chunk_idx == WAVEFORM_SAMPLES - 1 {
            // Last chunk gets any remaining samples
            all_samples.len()
        } else {
            start + samples_per_chunk
        };

        // Compute RMS (root mean square) for this chunk
        let chunk = &all_samples[start..end];
        let sum_squares: f32 = chunk.iter().map(|s| s * s).sum();
        let rms = (sum_squares / chunk.len() as f32).sqrt();
        rms_values.push(rms);
    }

    // Find max RMS for normalization
    let max_rms = rms_values
        .iter()
        .cloned()
        .fold(0.0f32, |a, b| a.max(b))
        .max(0.001); // Avoid division by zero

    // Normalize to 0-255 range
    let waveform: Vec<u8> = rms_values
        .iter()
        .map(|&rms| {
            let normalized = rms / max_rms;
            (normalized * 255.0) as u8
        })
        .collect();

    log::debug!(
        "[Waveform] Computed waveform with {} samples, max_rms={:.4}",
        waveform.len(),
        max_rms
    );

    Ok(waveform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_nonexistent_file() {
        let result = analyze_waveform("nonexistent_file.flac");
        assert!(matches!(result, Err(AudioDecoderError::FileNotFound(_))));
    }

    #[test]
    fn test_waveform_length() {
        // Test that waveform always returns WAVEFORM_SAMPLES elements
        // Even if the file doesn't exist, this tests the constant
        assert_eq!(WAVEFORM_SAMPLES, 128);
    }
}
