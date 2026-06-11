//! Spectral descriptors: centroid, rolloff, flatness.
//!
//! Replaces aubio PVoc + SpecDesc with a pure Rust Hann-windowed FFT.

use super::utils::{geometric_mean, mean, normalize, std_deviation};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

const WINDOW_SIZE: usize = 512;
const HOP_SIZE: usize = WINDOW_SIZE / 4; // 128

/// Compute spectral centroid, rolloff, and flatness.
///
/// Returns `[centroid_mean, centroid_std, rolloff_mean, rolloff_std, flatness_mean, flatness_std]`
/// all normalized to [-1, 1].
pub fn compute_spectral_features(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let half_sr = sr / 2.0;
    let n_bins = WINDOW_SIZE / 2 + 1; // 257

    // Pre-compute Hann window
    let hann: Vec<f32> = (0..WINDOW_SIZE)
        .map(|n| 0.5 - 0.5 * f32::cos(2.0 * PI * n as f32 / WINDOW_SIZE as f32))
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);

    let mut values_centroid = Vec::new();
    let mut values_rolloff = Vec::new();
    let mut values_flatness = Vec::new();

    // Process with overlap (window=512, hop=128)
    for chunk in samples.windows(WINDOW_SIZE).step_by(HOP_SIZE) {
        // Apply Hann window and FFT
        let mut buffer: Vec<Complex<f32>> = chunk
            .iter()
            .zip(hann.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();
        fft.process(&mut buffer);

        // Magnitude spectrum (n_bins = 257)
        // aubio PVoc produces a CVec with norm[0..n_bins] — we match that
        let norms: Vec<f32> = buffer[..n_bins].iter().map(|c| c.norm()).collect();

        // --- Centroid ---
        // aubio centroid: sum(bin * mag) / sum(mag) → returns bin index
        // then bin_to_freq converts to Hz
        let sum_mag: f32 = norms.iter().sum();
        let centroid_bin = if sum_mag > 0.0 {
            norms
                .iter()
                .enumerate()
                .map(|(i, &m)| i as f32 * m)
                .sum::<f32>()
                / sum_mag
        } else {
            0.0
        };
        let centroid_freq = centroid_bin * sr / WINDOW_SIZE as f32;
        values_centroid.push(centroid_freq);

        // --- Rolloff ---
        // aubio rolloff: find bin at 95% cumulative energy, clamp to WINDOW_SIZE/2
        let total_energy: f32 = norms.iter().map(|&m| m * m).sum();
        let threshold = 0.95 * total_energy;
        let mut cumulative = 0.0;
        let mut rolloff_bin = 0.0_f32;
        for (i, &m) in norms.iter().enumerate() {
            cumulative += m * m;
            if cumulative >= threshold {
                rolloff_bin = i as f32;
                break;
            }
        }
        // Clamp like aubio bug workaround
        if rolloff_bin > WINDOW_SIZE as f32 / 2.0 {
            rolloff_bin = WINDOW_SIZE as f32 / 2.0;
        }
        let rolloff_freq = rolloff_bin * sr / WINDOW_SIZE as f32;
        values_rolloff.push(rolloff_freq);

        // --- Flatness ---
        // aubio flatness: geometric_mean(cvec.norm()) / mean(cvec.norm())
        // geometric_mean needs a multiple of 8, so use the largest multiple of 8
        // <= norms.len() instead of a hardcoded value.
        let geo_len = (norms.len() / 8) * 8;
        let geo = geometric_mean(&norms[..geo_len]);
        if geo == 0.0 {
            values_flatness.push(0.0);
        } else {
            let flatness = geo / mean(&norms);
            values_flatness.push(flatness);
        }
    }

    // Normalize centroid/rolloff to [-1, 1] with range [0, sr/2]
    let centroid_mean = normalize(mean(&values_centroid), 0.0, half_sr);
    let centroid_std = normalize(std_deviation(&values_centroid), 0.0, half_sr);
    let rolloff_mean = normalize(mean(&values_rolloff), 0.0, half_sr);
    let rolloff_std = normalize(std_deviation(&values_rolloff), 0.0, half_sr);

    // Flatness is already in [0, 1], normalize to [-1, 1]
    let flatness_mean = normalize(mean(&values_flatness), 0.0, 1.0);
    let flatness_std = normalize(std_deviation(&values_flatness), 0.0, 1.0);

    vec![
        centroid_mean,
        centroid_std,
        rolloff_mean,
        rolloff_std,
        flatness_mean,
        flatness_std,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectral_silence() {
        let silence = vec![0.0; 1024];
        let features = compute_spectral_features(&silence, 22050);
        assert_eq!(features.len(), 6);
        // All should be -1 for silence
        for &f in &features {
            assert!(f <= -0.99, "expected ~-1 for silence, got {f}");
        }
    }

    #[test]
    fn test_spectral_features_length() {
        let signal: Vec<f32> = (0..22050)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 22050.0).sin())
            .collect();
        let features = compute_spectral_features(&signal, 22050);
        assert_eq!(features.len(), 6);
        // All values should be in [-1, 1]
        for &f in &features {
            assert!((-1.0..=1.0).contains(&f), "feature out of range: {f}");
        }
    }
}

#[test]
fn test_spectral_too_short() {
    let short = vec![0.5f32; 100];
    let features = compute_spectral_features(&short, 22050);
    assert_eq!(features.len(), 6);
    for &f in &features {
        assert!(
            (f - -1.0).abs() < 1e-6,
            "expected -1 for no windows, got {f}"
        );
    }
}
