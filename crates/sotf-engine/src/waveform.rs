pub use math_audio_dsp::waveform::{WAVEFORM_SAMPLES, compute_waveform};

use crate::decoder::{AudioDecoderResult, create_decoder};
use std::path::Path;

/// Analyze an audio file and compute a waveform representation.
///
/// Uses streaming computation to avoid loading the entire file into memory.
/// Processes audio in chunks, computing RMS values incrementally.
pub fn analyze_waveform<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Vec<u8>> {
    log::debug!(
        "[Waveform] Analyze: {}",
        path.as_ref().to_str().unwrap_or("unknown")
    );

    let mut decoder = create_decoder(path.as_ref())?;

    // Get total frames if available for optimal chunking
    let total_frames = decoder.spec().total_frames;

    // First pass: count total mono frames if not available from metadata
    let total_mono_frames = if let Some(frames) = total_frames {
        frames as usize
    } else {
        // Need to count frames - but we can't seek back easily
        // Fall back to the legacy approach for files without frame count
        return analyze_waveform_legacy(path);
    };

    // Calculate frames per waveform chunk
    let frames_per_chunk = total_mono_frames / WAVEFORM_SAMPLES;
    if frames_per_chunk == 0 {
        // Very short file - use legacy approach
        return analyze_waveform_legacy(path);
    }

    // Streaming computation: accumulate sum of squares for each chunk
    let mut rms_values: Vec<f32> = Vec::with_capacity(WAVEFORM_SAMPLES);
    let mut current_chunk_sum_squares: f64 = 0.0;
    let mut current_chunk_count: usize = 0;
    let mut total_frames_processed: usize = 0;
    let mut current_chunk_idx: usize = 0;

    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }

        let channels = decoded.spec.channels as usize;
        let samples = &decoded.samples;

        for frame_start in (0..samples.len()).step_by(channels) {
            // Mix to mono
            let mut mono_sum = 0.0f32;
            for ch in 0..channels {
                if frame_start + ch < samples.len() {
                    mono_sum += samples[frame_start + ch];
                }
            }
            let mono_sample = mono_sum / channels as f32;

            // Accumulate for current chunk
            current_chunk_sum_squares += (mono_sample as f64) * (mono_sample as f64);
            current_chunk_count += 1;
            total_frames_processed += 1;

            // Check if we've completed a chunk
            let target_frame = (current_chunk_idx + 1) * frames_per_chunk;
            if total_frames_processed >= target_frame && current_chunk_idx < WAVEFORM_SAMPLES - 1 {
                // Compute RMS for this chunk
                let rms = if current_chunk_count > 0 {
                    (current_chunk_sum_squares / current_chunk_count as f64).sqrt() as f32
                } else {
                    0.0
                };
                rms_values.push(rms);

                // Reset for next chunk
                current_chunk_sum_squares = 0.0;
                current_chunk_count = 0;
                current_chunk_idx += 1;
            }
        }
    }

    // Finalize the last chunk (may include extra samples due to rounding)
    if current_chunk_count > 0 || rms_values.len() < WAVEFORM_SAMPLES {
        let rms = if current_chunk_count > 0 {
            (current_chunk_sum_squares / current_chunk_count as f64).sqrt() as f32
        } else {
            0.0
        };
        rms_values.push(rms);
    }

    // Pad if we don't have enough values (shouldn't happen normally)
    while rms_values.len() < WAVEFORM_SAMPLES {
        rms_values.push(0.0);
    }

    // Normalize to 0-255
    let max_rms = rms_values
        .iter()
        .cloned()
        .fold(0.0f32, |a, b| a.max(b))
        .max(0.001);

    let waveform: Vec<u8> = rms_values
        .iter()
        .take(WAVEFORM_SAMPLES)
        .map(|&rms| {
            let normalized = rms / max_rms;
            (normalized * 255.0) as u8
        })
        .collect();

    log::debug!(
        "[Waveform] Computed streaming waveform: {} frames, max_rms={:.4}",
        total_frames_processed,
        max_rms
    );

    Ok(waveform)
}

const LEGACY_RMS_WINDOW_FRAMES: usize = 1024;

/// Fallback waveform analysis for files without frame count metadata.
///
/// Keeps only per-window RMS values instead of the full mono sample stream, so
/// memory use grows with file duration / window size rather than sample count.
fn analyze_waveform_legacy<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Vec<u8>> {
    log::debug!(
        "[Waveform] Windowed analyze (no frame count): {}",
        path.as_ref().to_str().unwrap_or("unknown")
    );

    let mut decoder = create_decoder(path.as_ref())?;

    let mut rms_windows: Vec<f32> = Vec::new();
    let mut sum_squares = 0.0f64;
    let mut count = 0usize;

    while let Some(decoded) = decoder.decode_next()? {
        if decoded.is_empty() {
            continue;
        }
        let channels = decoded.spec.channels as usize;
        let samples = &decoded.samples;
        for frame_start in (0..samples.len()).step_by(channels) {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                if frame_start + ch < samples.len() {
                    sum += samples[frame_start + ch];
                }
            }
            let mono = sum / channels as f32;
            sum_squares += (mono as f64) * (mono as f64);
            count += 1;

            if count >= LEGACY_RMS_WINDOW_FRAMES {
                rms_windows.push((sum_squares / count as f64).sqrt() as f32);
                sum_squares = 0.0;
                count = 0;
            }
        }
    }

    if count > 0 {
        rms_windows.push((sum_squares / count as f64).sqrt() as f32);
    }

    Ok(compute_waveform(&rms_windows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::AudioDecoderError;

    #[test]
    fn test_analyze_nonexistent_file() {
        let result = analyze_waveform("nonexistent_file.flac");
        assert!(matches!(result, Err(AudioDecoderError::FileNotFound(_))));
    }

    #[test]
    fn test_waveform_length() {
        assert_eq!(WAVEFORM_SAMPLES, 128);
    }
}
