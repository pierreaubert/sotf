//! Weighted loss functions for perceptual frequency weighting.
//!
//! Implements frequency-dependent weighting for loss calculations to better
//! match human hearing characteristics.

#![allow(dead_code)]

use ndarray::Array1;

/// Frequency weighting type for loss calculations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrequencyWeighting {
    /// No weighting - all frequencies weighted equally
    Flat,
    /// A-weighting - matches human hearing at moderate levels
    AWeighting,
    /// Bass emphasis - higher weight below 200 Hz
    BassEmphasis,
    /// Custom frequency-dependent weighting
    Custom,
}

/// Configuration for weighted loss calculation
#[derive(Debug, Clone)]
pub struct WeightedLossConfig {
    /// Type of frequency weighting
    pub weighting: FrequencyWeighting,
    /// Bass emphasis factor (for BassEmphasis or Custom)
    pub bass_emphasis: f64,
    /// Midrange emphasis factor
    pub midrange_emphasis: f64,
    /// Treble emphasis factor
    pub treble_emphasis: f64,
    /// Custom weight bands: (low_freq, high_freq, weight)
    pub custom_bands: Vec<(f64, f64, f64)>,
}

impl Default for WeightedLossConfig {
    fn default() -> Self {
        Self {
            weighting: FrequencyWeighting::Flat,
            bass_emphasis: 1.0,
            midrange_emphasis: 1.0,
            treble_emphasis: 1.0,
            custom_bands: Vec::new(),
        }
    }
}

/// Compute A-weighting coefficient for a given frequency.
///
/// A-weighting approximates human hearing sensitivity at moderate SPL levels.
/// Reference: IEC 61672-1:2013
///
/// # Arguments
/// * `freq` - Frequency in Hz
///
/// # Returns
/// * Linear weighting factor (not dB)
pub fn a_weighting_linear(freq: f64) -> f64 {
    // A-weighting formula
    // R_A(f) = (12194^2 * f^4) / ((f^2 + 20.6^2) * sqrt((f^2 + 107.7^2)(f^2 + 737.9^2)) * (f^2 + 12194^2))

    let f2 = freq * freq;
    let f4 = f2 * f2;

    let c1 = 12194.0_f64.powi(2);
    let c2 = 20.6_f64.powi(2);
    let c3 = 107.7_f64.powi(2);
    let c4 = 737.9_f64.powi(2);

    let num = c1 * f4;
    let den1 = f2 + c2;
    let den2 = ((f2 + c3) * (f2 + c4)).sqrt();
    let den3 = f2 + c1;

    let ra = num / (den1 * den2 * den3);

    // Normalize so that 1 kHz has weight 1.0
    let ra_1k = {
        let f_1k = 1000.0;
        let f2_1k = f_1k * f_1k;
        let f4_1k = f2_1k * f2_1k;
        let num_1k = c1 * f4_1k;
        let den1_1k = f2_1k + c2;
        let den2_1k = ((f2_1k + c3) * (f2_1k + c4)).sqrt();
        let den3_1k = f2_1k + c1;
        num_1k / (den1_1k * den2_1k * den3_1k)
    };

    ra / ra_1k
}

/// Compute A-weighting in dB for a given frequency.
pub fn a_weighting_db(freq: f64) -> f64 {
    20.0 * a_weighting_linear(freq).log10()
}

/// Compute frequency weights for an array of frequencies.
///
/// # Arguments
/// * `freq` - Array of frequencies in Hz
/// * `config` - Weighting configuration
///
/// # Returns
/// * Array of linear weights (multiply errors by these values)
pub fn compute_weights(freq: &Array1<f64>, config: &WeightedLossConfig) -> Array1<f64> {
    match config.weighting {
        FrequencyWeighting::Flat => Array1::ones(freq.len()),

        FrequencyWeighting::AWeighting => freq.map(|&f| a_weighting_linear(f)),

        FrequencyWeighting::BassEmphasis => freq.map(|&f| {
            if f < 200.0 {
                config.bass_emphasis
            } else if f < 2000.0 {
                config.midrange_emphasis
            } else {
                config.treble_emphasis
            }
        }),

        FrequencyWeighting::Custom => {
            freq.map(|&f| {
                // Find matching custom band
                for &(low, high, weight) in &config.custom_bands {
                    if f >= low && f < high {
                        return weight;
                    }
                }
                // Default weight if no band matches
                1.0
            })
        }
    }
}

/// Calculate weighted RMS error.
///
/// # Arguments
/// * `error` - Error values (e.g., deviation from target in dB)
/// * `weights` - Per-frequency weights
///
/// # Returns
/// * Weighted RMS error
pub fn weighted_rms_error(error: &Array1<f64>, weights: &Array1<f64>) -> f64 {
    let weighted_sq: f64 = error
        .iter()
        .zip(weights.iter())
        .map(|(&e, &w)| e * e * w)
        .sum();
    let total_weight: f64 = weights.iter().sum();

    if total_weight > 0.0 {
        (weighted_sq / total_weight).sqrt()
    } else {
        0.0
    }
}

/// Calculate weighted mean absolute error.
///
/// # Arguments
/// * `error` - Error values (e.g., deviation from target in dB)
/// * `weights` - Per-frequency weights
///
/// # Returns
/// * Weighted MAE
pub fn weighted_mae(error: &Array1<f64>, weights: &Array1<f64>) -> f64 {
    let weighted_abs: f64 = error
        .iter()
        .zip(weights.iter())
        .map(|(&e, &w)| e.abs() * w)
        .sum();
    let total_weight: f64 = weights.iter().sum();

    if total_weight > 0.0 {
        weighted_abs / total_weight
    } else {
        0.0
    }
}

/// Calculate weighted loss combining RMS and peak error.
///
/// This is useful for penalizing both overall deviation and sharp peaks/dips.
///
/// # Arguments
/// * `error` - Error values
/// * `weights` - Per-frequency weights
/// * `peak_weight` - How much to weight the peak error (0.0 to 1.0)
///
/// # Returns
/// * Combined weighted loss
pub fn weighted_combined_loss(error: &Array1<f64>, weights: &Array1<f64>, peak_weight: f64) -> f64 {
    let rms = weighted_rms_error(error, weights);

    // Find peak weighted error
    let peak = error
        .iter()
        .zip(weights.iter())
        .map(|(&e, &w)| e.abs() * w.sqrt()) // sqrt to moderate weight effect on peak
        .fold(0.0_f64, f64::max);

    (1.0 - peak_weight) * rms + peak_weight * peak
}

/// Create standard bass-emphasis configuration for room EQ.
///
/// This configuration gives extra weight to bass frequencies where
/// room modes are most problematic.
pub fn bass_emphasis_config() -> WeightedLossConfig {
    WeightedLossConfig {
        weighting: FrequencyWeighting::Custom,
        bass_emphasis: 2.0,
        midrange_emphasis: 1.0,
        treble_emphasis: 0.5,
        custom_bands: vec![
            (20.0, 80.0, 2.5),      // Deep bass - highest weight
            (80.0, 200.0, 2.0),     // Upper bass
            (200.0, 500.0, 1.5),    // Lower midrange
            (500.0, 2000.0, 1.0),   // Midrange
            (2000.0, 8000.0, 0.8),  // Presence
            (8000.0, 20000.0, 0.5), // Treble
        ],
    }
}

/// Create A-weighted configuration.
pub fn a_weighted_config() -> WeightedLossConfig {
    WeightedLossConfig {
        weighting: FrequencyWeighting::AWeighting,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert that two floats are approximately equal
    fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
        assert!(
            (a - b).abs() < epsilon,
            "assertion failed: {} ≈ {} (diff = {}, epsilon = {})",
            a,
            b,
            (a - b).abs(),
            epsilon
        );
    }

    #[test]
    fn test_a_weighting_1khz() {
        // A-weighting should be 0 dB at 1 kHz
        let db = a_weighting_db(1000.0);
        assert_approx_eq(db, 0.0, 0.1);
    }

    #[test]
    fn test_a_weighting_low_freq() {
        // A-weighting should be significantly negative at low frequencies
        let db_20 = a_weighting_db(20.0);
        let db_100 = a_weighting_db(100.0);

        assert!(db_20 < -40.0, "20 Hz should be < -40 dB, got {}", db_20);
        assert!(db_100 < -15.0, "100 Hz should be < -15 dB, got {}", db_100);
    }

    #[test]
    fn test_a_weighting_high_freq() {
        // A-weighting should be slightly negative at high frequencies
        let db_10k = a_weighting_db(10000.0);
        assert!(
            db_10k > -5.0 && db_10k < 5.0,
            "10 kHz should be near 0 dB, got {}",
            db_10k
        );
    }

    #[test]
    fn test_flat_weighting() {
        let freq = Array1::linspace(20.0, 20000.0, 100);
        let config = WeightedLossConfig::default();
        let weights = compute_weights(&freq, &config);

        assert!(weights.iter().all(|&w| (w - 1.0).abs() < 0.001));
    }

    #[test]
    fn test_weighted_rms() {
        let error = Array1::from_vec(vec![2.0, 4.0, 6.0]);
        let weights = Array1::from_vec(vec![1.0, 1.0, 1.0]);

        let rms = weighted_rms_error(&error, &weights);
        // sqrt((4 + 16 + 36) / 3) = sqrt(56/3) ≈ 4.32
        assert_approx_eq(rms, 4.32, 0.1);
    }

    #[test]
    fn test_weighted_mae() {
        let error = Array1::from_vec(vec![-2.0, 4.0, -6.0]);
        let weights = Array1::from_vec(vec![1.0, 1.0, 1.0]);

        let mae = weighted_mae(&error, &weights);
        // (2 + 4 + 6) / 3 = 4.0
        assert_approx_eq(mae, 4.0, 0.01);
    }

    #[test]
    fn test_bass_emphasis_config() {
        let config = bass_emphasis_config();
        let freq = Array1::from_vec(vec![50.0, 150.0, 1000.0, 10000.0]);
        let weights = compute_weights(&freq, &config);

        // Bass should have higher weight than treble
        assert!(weights[0] > weights[3]);
        assert!(weights[1] > weights[3]);
    }

    #[test]
    fn test_custom_bands() {
        let config = WeightedLossConfig {
            weighting: FrequencyWeighting::Custom,
            custom_bands: vec![
                (0.0, 100.0, 3.0),
                (100.0, 1000.0, 2.0),
                (1000.0, 20000.0, 1.0),
            ],
            ..Default::default()
        };

        let freq = Array1::from_vec(vec![50.0, 500.0, 5000.0]);
        let weights = compute_weights(&freq, &config);

        assert_approx_eq(weights[0], 3.0, 0.01);
        assert_approx_eq(weights[1], 2.0, 0.01);
        assert_approx_eq(weights[2], 1.0, 0.01);
    }
}
