//! FIR filter optimization for room correction
//!
//! This module provides high-level FIR correction generation for room EQ,
//! using the core FIR design functions from `math_audio_iir_fir`.

use crate::Curve;
use ndarray::Array1;
use std::error::Error;

use super::types::{OptimizerConfig, TargetCurveConfig};

// Re-export FirPhase from our fir module (which re-exports from math-iir-fir)
pub use crate::fir::FirPhase;

/// Generate an FIR correction filter for a single channel
///
/// This is the main entry point for FIR-based room correction. It handles:
/// - Target curve loading (from file path or predefined name)
/// - Phase type selection (linear, minimum, or kirkeby)
/// - FIR coefficient generation
///
/// # Arguments
/// * `measurement` - The room measurement curve
/// * `config` - Optimizer configuration (contains FIR settings)
/// * `target_config` - Optional target curve configuration
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// * Vector of FIR coefficients on success
pub fn generate_fir_correction(
    measurement: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
) -> Result<Vec<f64>, Box<dyn Error>> {
    // 1. Determine Target Curve
    let target_curve = match target_config {
        Some(TargetCurveConfig::Path(path)) => {
            let target = crate::read::read_curve_from_csv(path)?;
            crate::read::normalize_and_interpolate_response(&measurement.freq, &target)
        }
        Some(TargetCurveConfig::Predefined(name)) => {
            match crate::workflow::build_target_curve_by_name(name, &measurement.freq, measurement)
            {
                Ok(curve) => curve,
                Err(_) => {
                    // Fallback to file path
                    let target = crate::read::read_curve_from_csv(&std::path::PathBuf::from(name))?;
                    crate::read::normalize_and_interpolate_response(&measurement.freq, &target)
                }
            }
        }
        None => {
            // Default target: flat at measurement's mean level (within the optimization band)
            // This centers corrections around 0 dB, making boost/cut limits work properly
            let min_freq = config.min_freq;
            let max_freq = config.max_freq;
            let mut sum = 0.0;
            let mut count = 0;
            for i in 0..measurement.freq.len() {
                if measurement.freq[i] >= min_freq && measurement.freq[i] <= max_freq {
                    sum += measurement.spl[i];
                    count += 1;
                }
            }
            let mean_level = if count > 0 { sum / count as f64 } else { 0.0 };

            Curve {
                freq: measurement.freq.clone(),
                spl: Array1::from_elem(measurement.freq.len(), mean_level),
                phase: None,
                ..Default::default()
            }
        }
    };

    let fir_config = config.fir.as_ref().ok_or("FIR configuration missing")?;
    let n_taps = fir_config.taps;

    if fir_config.phase.to_lowercase() == "kirkeby" {
        // Use Kirkeby regularized inversion with optional excess phase correction
        let coeffs = crate::fir::generate_kirkeby_correction_with_smoothing(
            measurement,
            &target_curve,
            sample_rate,
            n_taps,
            config.min_freq,
            config.max_freq,
            fir_config.correct_excess_phase,
            fir_config.phase_smoothing,
        );
        Ok(coeffs)
    } else {
        // Standard magnitude-based generation
        let correction_spl = &target_curve.spl - &measurement.spl;
        let correction_curve = Curve {
            freq: measurement.freq.clone(),
            spl: correction_spl,
            phase: None,
            ..Default::default()
        };

        let phase_type = match fir_config.phase.to_lowercase().as_str() {
            "linear" => FirPhase::Linear,
            "minimum" => FirPhase::Minimum,
            _ => return Err(format!("Unknown FIR phase type: {}", fir_config.phase).into()),
        };

        // Convert pre-ringing config if present
        let pre_ringing =
            fir_config
                .pre_ringing
                .as_ref()
                .map(|pr| math_audio_iir_fir::PreRingingConfig {
                    threshold_db: pr.threshold_db,
                    max_time_s: pr.max_time_s,
                });

        let fir_design_config = math_audio_iir_fir::FirDesignConfig {
            n_taps,
            sample_rate,
            phase: phase_type,
            pre_ringing,
            ..Default::default()
        };

        let freqs: Vec<f64> = correction_curve.freq.to_vec();
        let magnitude_db: Vec<f64> = correction_curve.spl.to_vec();
        let coeffs = math_audio_iir_fir::generate_fir_from_response(
            &freqs,
            &magnitude_db,
            &fir_design_config,
        );
        Ok(coeffs)
    }
}

/// Generate an FIR correction filter with optional GD alignment target (GD-3b).
///
/// When `gd_target` is `Some`, the per-channel delay from the GD-Opt result is
/// incorporated into the FIR design as additional phase correction. This allows
/// `PhaseLinear` mode to achieve inter-channel GD alignment without IIR AP filters.
///
/// When `gd_target` is `None`, this is equivalent to `generate_fir_correction`.
pub fn generate_fir_correction_with_gd_target(
    measurement: &Curve,
    config: &OptimizerConfig,
    target_config: Option<&TargetCurveConfig>,
    sample_rate: f64,
    gd_target: Option<&super::gd_opt::GdAlignmentTarget>,
    channel_index: usize,
) -> Result<Vec<f64>, Box<dyn Error>> {
    // Generate base FIR correction
    let mut coeffs = generate_fir_correction(measurement, config, target_config, sample_rate)?;

    // If a GD alignment target is provided, apply the per-channel delay
    // as a time-domain shift of the FIR coefficients.
    if let Some(delay_ms) = gd_target
        .and_then(|t| t.per_channel_delay_ms.get(channel_index))
        .copied()
        .filter(|d| d.abs() > 1e-6)
    {
        let delay_samples = delay_ms * 1e-3 * sample_rate;
        coeffs = apply_fractional_sample_shift(&coeffs, delay_samples);
    }

    Ok(coeffs)
}

/// Apply a GD-alignment delay to existing FIR coefficients.
///
/// This is used when a production path has already generated FIRs before the
/// room-level GD target is known. It mirrors the delay handling in
/// `generate_fir_correction_with_gd_target` so both paths encode the same
/// sample-domain shift into the convolution IR.
pub(crate) fn apply_gd_delay_to_fir_coefficients(
    coeffs: &[f64],
    delay_ms: f64,
    sample_rate: f64,
) -> Vec<f64> {
    if delay_ms.abs() <= 1e-6 {
        return coeffs.to_vec();
    }
    let delay_samples = delay_ms * 1e-3 * sample_rate;
    apply_fractional_sample_shift(coeffs, delay_samples)
}

/// Shift FIR coefficients by a given number of samples (positive = later).
/// Pads with zeros on the appropriate side and truncates to maintain length.
fn apply_sample_shift(coeffs: &[f64], shift: isize) -> Vec<f64> {
    let n = coeffs.len();
    let mut shifted = vec![0.0; n];

    if shift >= 0 {
        let s = shift as usize;
        if s < n {
            shifted[s..n].copy_from_slice(&coeffs[..(n - s)]);
        }
    } else {
        let s = (-shift) as usize;
        let len = n.saturating_sub(s);
        if len > 0 {
            shifted[..len].copy_from_slice(&coeffs[s..s + len]);
        }
    }

    shifted
}

/// Shift FIR coefficients by a fractional number of samples using linear
/// interpolation. Positive shift = later (delays the signal).
fn apply_fractional_sample_shift(coeffs: &[f64], shift: f64) -> Vec<f64> {
    let n = coeffs.len();
    if shift.abs() < 1e-9 {
        return coeffs.to_vec();
    }
    let mut shifted = vec![0.0; n];
    for i in 0..n {
        let src = i as f64 - shift;
        let idx = src.floor();
        let frac = src - idx;
        let idx = idx as isize;
        if idx < 0 || idx >= n as isize {
            continue;
        }
        let v0 = coeffs[idx as usize];
        if frac.abs() < 1e-9 || idx + 1 >= n as isize {
            shifted[i] = v0;
        } else {
            let v1 = coeffs[(idx + 1) as usize];
            shifted[i] = (1.0 - frac) * v0 + frac * v1;
        }
    }
    shifted
}

#[cfg(test)]
pub use math_audio_iir_fir::{WindowType, generate_window};
#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::roomeq::types::FirConfig;
    use ndarray::Array1;

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

    /// Helper to create a test curve
    fn create_test_curve(freqs: &[f64], spl_values: &[f64]) -> Curve {
        Curve {
            freq: Array1::from(freqs.to_vec()),
            spl: Array1::from(spl_values.to_vec()),
            phase: None,
            ..Default::default()
        }
    }

    /// Create a curve with phase data
    fn create_test_curve_with_phase(freqs: &[f64], spl_values: &[f64], phase_deg: &[f64]) -> Curve {
        Curve {
            freq: Array1::from(freqs.to_vec()),
            spl: Array1::from(spl_values.to_vec()),
            phase: Some(Array1::from(phase_deg.to_vec())),
            ..Default::default()
        }
    }

    // Window function tests - using the re-exported functions from math-iir-fir

    #[test]
    fn test_hann_window_symmetry() {
        let window = generate_window(8, WindowType::Hann, 0.0);
        assert_approx_eq(window[0], window[7], 0.01);
        assert_approx_eq(window[1], window[6], 0.01);
        assert_approx_eq(window[2], window[5], 0.01);
        assert_approx_eq(window[3], window[4], 0.01);
    }

    #[test]
    fn test_hann_window_endpoints() {
        let window = generate_window(128, WindowType::Hann, 0.0);
        // Hann should be 0 at endpoints
        assert!(window[0] < 0.01);
        assert!(window[127] < 0.01);
        // Maximum should be at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_hamming_window_endpoints() {
        let window = generate_window(128, WindowType::Hamming, 0.0);
        // Hamming has non-zero endpoints (~0.08)
        assert!(window[0] > 0.07 && window[0] < 0.09);
        // Maximum at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_blackman_window_endpoints() {
        let window = generate_window(128, WindowType::Blackman, 0.0);
        // Blackman should be very close to 0 at endpoints
        assert!(window[0] < 0.01);
        // Maximum at center
        assert!(window[64] > 0.99);
    }

    #[test]
    fn test_kaiser_window_beta_0() {
        // beta = 0 should give rectangular window
        let window = generate_window(8, WindowType::Kaiser, 0.0);
        for w in window {
            assert_approx_eq(w, 1.0, 0.01);
        }
    }

    #[test]
    fn test_rectangular_window() {
        let window = generate_window(10, WindowType::Rectangular, 0.0);
        assert_eq!(window.len(), 10);
        for w in window {
            assert_eq!(w, 1.0);
        }
    }

    // FIR correction tests

    #[test]
    fn test_kirkeby_with_phase_data() {
        let freqs = vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        let spl = vec![75.0, 80.0, 85.0, 82.0, 80.0, 78.0, 76.0, 74.0, 70.0, 65.0];
        let phase = vec![
            -180.0, -120.0, -60.0, -30.0, 0.0, 30.0, 60.0, 90.0, 120.0, 150.0,
        ];

        let measurement = create_test_curve_with_phase(&freqs, &spl, &phase);

        let target = create_test_curve(
            &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
            &[80.0, 80.0, 80.0, 80.0, 80.0],
        );

        let coeffs = crate::fir::generate_kirkeby_correction(
            &measurement,
            &target,
            48000.0,
            4096,
            20.0,
            1000.0,
        );

        assert_eq!(coeffs.len(), 4096);
        assert!(coeffs.iter().any(|&x| x.abs() > 1e-10));
    }

    #[test]
    fn test_kirkeby_without_phase_data() {
        let measurement = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[75.0, 82.0, 80.0, 78.0, 72.0, 65.0],
        );

        let target = create_test_curve(
            &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
            &[80.0, 80.0, 80.0, 80.0, 80.0],
        );

        let coeffs = crate::fir::generate_kirkeby_correction(
            &measurement,
            &target,
            48000.0,
            4096,
            20.0,
            1000.0,
        );

        assert_eq!(coeffs.len(), 4096);
    }

    #[test]
    fn test_generate_fir_correction_basic() {
        let measurement = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[78.0, 82.0, 80.0, 79.0, 75.0, 70.0],
        );

        let mut config = OptimizerConfig::default();
        config.fir = Some(FirConfig {
            taps: 1024,
            phase: "linear".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        });
        config.min_freq = 50.0;
        config.max_freq = 2000.0;

        let result = generate_fir_correction(&measurement, &config, None, 48000.0);

        assert!(
            result.is_ok(),
            "FIR correction should succeed: {:?}",
            result.err()
        );
        let coeffs = result.unwrap();
        assert_eq!(coeffs.len(), 1024);
    }

    #[test]
    fn test_generate_fir_correction_kirkeby_mode() {
        let measurement = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[78.0, 82.0, 80.0, 79.0, 75.0, 70.0],
        );

        let mut config = OptimizerConfig::default();
        config.fir = Some(FirConfig {
            taps: 2048,
            phase: "kirkeby".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        });
        config.min_freq = 20.0;
        config.max_freq = 500.0;

        let result = generate_fir_correction(&measurement, &config, None, 48000.0);

        assert!(result.is_ok(), "Kirkeby FIR correction should succeed");
        let coeffs = result.unwrap();
        assert_eq!(coeffs.len(), 2048);
    }

    #[test]
    fn test_fir_config_missing_returns_error() {
        let measurement = create_test_curve(&[20.0, 1000.0, 20000.0], &[80.0, 80.0, 80.0]);

        let config = OptimizerConfig::default(); // fir is None by default

        let result = generate_fir_correction(&measurement, &config, None, 48000.0);

        assert!(result.is_err(), "Should error when FIR config is missing");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("FIR configuration missing"),
            "Error should mention missing FIR config"
        );
    }

    #[test]
    fn test_invalid_phase_type_returns_error() {
        let measurement = create_test_curve(&[20.0, 1000.0, 20000.0], &[80.0, 80.0, 80.0]);

        let mut config = OptimizerConfig::default();
        config.fir = Some(FirConfig {
            taps: 1024,
            phase: "invalid_phase_type".to_string(),
            correct_excess_phase: false,
            phase_smoothing: 0.167,
            pre_ringing: None,
        });

        let result = generate_fir_correction(&measurement, &config, None, 48000.0);

        assert!(result.is_err(), "Should error on invalid phase type");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Unknown FIR phase type"),
            "Error should mention unknown phase type"
        );
    }

    #[test]
    fn fractional_delay_preserves_impulse_shape_better_than_rounding() {
        // An impulse at sample 5: [0,0,0,0,0,1,0,0,0,0]
        let coeffs: Vec<f64> = (0..10).map(|i| if i == 5 { 1.0 } else { 0.0 }).collect();
        // Target delay: 2.3 samples
        let delay_ms = 2.3 / 48000.0 * 1000.0;

        // Old integer-rounding method
        let old_samples = (delay_ms * 1e-3 * 48000.0f64).round() as isize;
        let old_shifted = super::apply_sample_shift(&coeffs, old_samples);

        // New fractional method
        let new_shifted = super::apply_fractional_sample_shift(&coeffs, 2.3);

        // The integer method rounds to 2 samples, so impulse is at index 7
        assert_eq!(old_shifted[7], 1.0);
        assert_eq!(old_shifted[6], 0.0);

        // The fractional method should spread the impulse between 7 and 8
        assert!(
            new_shifted[7] > 0.6 && new_shifted[7] < 0.8,
            "fractional delay should place 0.7 of impulse at idx 7, got {}",
            new_shifted[7]
        );
        assert!(
            new_shifted[8] > 0.2 && new_shifted[8] < 0.4,
            "fractional delay should place 0.3 of impulse at idx 8, got {}",
            new_shifted[8]
        );
    }

    #[test]
    fn fractional_delay_is_zero_for_integer_shift() {
        let coeffs: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let shifted = super::apply_fractional_sample_shift(&coeffs, 2.0);
        // Exactly 2.0 samples should be identical to integer shift
        assert_eq!(shifted[0], 0.0);
        assert_eq!(shifted[1], 0.0);
        assert_eq!(shifted[2], 1.0);
        assert_eq!(shifted[3], 2.0);
        assert_eq!(shifted[4], 3.0);
    }

    #[test]
    fn fractional_delay_handles_negative_shift() {
        let coeffs: Vec<f64> = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        // Negative shift = advance by 1.5 samples
        let shifted = super::apply_fractional_sample_shift(&coeffs, -1.5);
        // Original impulse at idx 3; after advancing 1.5, it should be at idx 1.5
        // So we expect energy at idx 1 and 2
        assert!(
            shifted[1] > 0.4 && shifted[1] < 0.6,
            "expected ~0.5 at idx 1, got {}",
            shifted[1]
        );
        assert!(
            shifted[2] > 0.4 && shifted[2] < 0.6,
            "expected ~0.5 at idx 2, got {}",
            shifted[2]
        );
    }
}
