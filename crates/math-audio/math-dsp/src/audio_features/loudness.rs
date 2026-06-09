//! Loudness descriptor.
//!
//! Replaces aubio `level_lin` with simple RMS computation.

use super::utils::{mean, normalize, std_deviation};

const WINDOW_SIZE: usize = 1024;
const MAX_VALUE: f32 = 0.0;
const MIN_VALUE: f32 = -90.0;

/// Compute loudness features (mean and std deviation of RMS in dB).
///
/// Returns `[loudness_mean, loudness_std]` normalized to [-1, 1].
pub fn compute_loudness(samples: &[f32]) -> Vec<f32> {
    let mut rms_values = Vec::new();

    for chunk in samples.chunks(WINDOW_SIZE) {
        // aubio level_lin = sqrt(sum(x^2) / n) — this is RMS
        let sum_sq: f32 = chunk.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / chunk.len() as f32).sqrt();
        rms_values.push(rms);
    }

    let mut mean_value = mean(&rms_values);
    let mut std_value = std_deviation(&rms_values);

    // Clamp to avoid log10(0)
    if mean_value < 1e-9 {
        mean_value = 1e-9;
    }
    if std_value < 1e-9 {
        std_value = 1e-9;
    }

    vec![
        normalize(10.0 * mean_value.log10(), MIN_VALUE, MAX_VALUE),
        normalize(10.0 * std_value.log10(), MIN_VALUE, MAX_VALUE),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudness_silence() {
        let silence = vec![0.0; 1024];
        let values = compute_loudness(&silence);
        assert_eq!(values.len(), 2);
        assert!((-1.0 - values[0]).abs() < 1e-6);
        assert!((-1.0 - values[1]).abs() < 1e-6);
    }

    #[test]
    fn test_loudness_full_scale() {
        let full = vec![1.0; 1024];
        let values = compute_loudness(&full);
        assert_eq!(values.len(), 2);
        // RMS of all 1.0 = 1.0, 10*log10(1.0) = 0 dB → normalized to 1.0
        assert!((1.0 - values[0]).abs() < 1e-6);
    }

    #[test]
    fn test_loudness_negative_full_scale() {
        let full = vec![-1.0; 1024];
        let values = compute_loudness(&full);
        // RMS of all -1.0 = 1.0
        assert!((1.0 - values[0]).abs() < 1e-6);
    }
}

#[test]
fn test_loudness_empty() {
    let values = compute_loudness(&[]);
    assert_eq!(values.len(), 2);
    assert!((values[0] - (-1.0)).abs() < 1e-6);
    assert!((values[1] - (-1.0)).abs() < 1e-6);
}
