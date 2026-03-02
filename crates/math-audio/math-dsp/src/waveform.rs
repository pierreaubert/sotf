/// Number of amplitude samples in the waveform
pub const WAVEFORM_SAMPLES: usize = 128;

/// Compute a waveform representation from mono audio samples.
///
/// Takes pre-decoded mono f32 samples and computes [`WAVEFORM_SAMPLES`] RMS
/// amplitude bins, each normalised to 0–255.
///
/// Returns a `Vec<u8>` of exactly [`WAVEFORM_SAMPLES`] elements.
pub fn compute_waveform(mono_samples: &[f32]) -> Vec<u8> {
    if mono_samples.is_empty() {
        log::warn!("[Waveform] No samples provided");
        return vec![0u8; WAVEFORM_SAMPLES];
    }

    let samples_per_chunk = mono_samples.len() / WAVEFORM_SAMPLES;

    // Handle case where input is shorter than WAVEFORM_SAMPLES
    if samples_per_chunk == 0 {
        log::warn!(
            "[Waveform] Input too short ({} samples), padding waveform",
            mono_samples.len()
        );
        let mut waveform = Vec::with_capacity(WAVEFORM_SAMPLES);
        for i in 0..WAVEFORM_SAMPLES {
            if i < mono_samples.len() {
                let amplitude = mono_samples[i].abs();
                waveform.push((amplitude.min(1.0) * 255.0) as u8);
            } else {
                waveform.push(0);
            }
        }
        return waveform;
    }

    // Compute RMS for each chunk
    let mut rms_values: Vec<f32> = Vec::with_capacity(WAVEFORM_SAMPLES);

    for chunk_idx in 0..WAVEFORM_SAMPLES {
        let start = chunk_idx * samples_per_chunk;
        let end = if chunk_idx == WAVEFORM_SAMPLES - 1 {
            mono_samples.len()
        } else {
            start + samples_per_chunk
        };

        let chunk = &mono_samples[start..end];
        let sum_squares: f32 = chunk.iter().map(|s| s * s).sum();
        let rms = (sum_squares / chunk.len() as f32).sqrt();
        rms_values.push(rms);
    }

    // Normalise to 0-255
    let max_rms = rms_values
        .iter()
        .cloned()
        .fold(0.0f32, |a, b| a.max(b))
        .max(0.001);

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

    waveform
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let waveform = compute_waveform(&[]);
        assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
        assert!(waveform.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_short_input() {
        let samples = vec![0.5; 10];
        let waveform = compute_waveform(&samples);
        assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
    }

    #[test]
    fn test_waveform_length_constant() {
        assert_eq!(WAVEFORM_SAMPLES, 128);
    }

    #[test]
    fn test_normal_input() {
        let samples: Vec<f32> = (0..48000)
            .map(|i| (i as f32 / 48000.0 * 440.0 * std::f32::consts::TAU).sin())
            .collect();
        let waveform = compute_waveform(&samples);
        assert_eq!(waveform.len(), WAVEFORM_SAMPLES);
        assert!(waveform.iter().any(|&v| v > 0));
    }
}
