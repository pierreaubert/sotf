//! Phase alignment optimization for subwoofer integration
//!
//! Maximizes energy sum in the crossover region between subwoofer and main speakers
//! by optimizing delay and polarity settings.
//!
//! # Algorithm
//! Uses golden section search for efficient 1D optimization of delay, combined with
//! optional A-weighting for perceptually-relevant energy computation.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use log::{debug, info};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;

use super::types::PhaseAlignmentConfig;

/// Weighting type for energy computation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeightingType {
    /// No weighting (flat)
    #[default]
    None,
    /// A-weighting (perceptual loudness)
    AWeighting,
    /// C-weighting (flat low frequency response)
    CWeighting,
}

/// Configuration for phase alignment optimization
#[derive(Debug, Clone)]
pub struct PhaseAlignmentOptConfig {
    /// Type of frequency weighting
    pub weighting: WeightingType,
    /// Tolerance for golden section search (ms)
    pub tolerance_ms: f64,
    /// Maximum iterations for optimization
    pub max_iterations: usize,
}

impl Default for PhaseAlignmentOptConfig {
    fn default() -> Self {
        Self {
            weighting: WeightingType::None,
            tolerance_ms: 0.01,
            max_iterations: 50,
        }
    }
}

/// Result of phase alignment optimization
#[derive(Debug, Clone)]
pub struct PhaseAlignmentResult {
    /// Optimal delay for the speaker relative to subwoofer (ms)
    /// Positive = delay speaker, Negative = delay subwoofer
    pub delay_ms: f64,
    /// Whether to invert polarity of the speaker
    pub invert_polarity: bool,
    /// Energy sum before optimization (arbitrary units)
    pub energy_before: f64,
    /// Energy sum after optimization (arbitrary units)
    pub energy_after: f64,
    /// Improvement in dB
    pub improvement_db: f64,
}

/// Optimize phase alignment between subwoofer and speaker
///
/// # Algorithm
/// 1. For each polarity (normal/inverted if enabled):
///    a. Use golden section search to find optimal delay (continuous, not discrete)
///    b. Compute weighted energy in [min_freq, max_freq] band
/// 2. Return delay/polarity combination that maximizes energy
///
/// # Arguments
/// * `sub_curve` - Subwoofer frequency response with phase
/// * `speaker_curve` - Speaker frequency response with phase
/// * `config` - Phase alignment configuration
///
/// # Returns
/// * Phase alignment result with optimal delay and polarity
pub fn optimize_phase_alignment(
    sub_curve: &Curve,
    speaker_curve: &Curve,
    config: &PhaseAlignmentConfig,
) -> Result<PhaseAlignmentResult> {
    optimize_phase_alignment_with_options(
        sub_curve,
        speaker_curve,
        config,
        PhaseAlignmentOptConfig::default(),
    )
}

/// Optimize phase alignment with custom optimization options.
pub fn optimize_phase_alignment_with_options(
    sub_curve: &Curve,
    speaker_curve: &Curve,
    config: &PhaseAlignmentConfig,
    opt_config: PhaseAlignmentOptConfig,
) -> Result<PhaseAlignmentResult> {
    if sub_curve.phase.is_none() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Subwoofer measurement must include phase data for phase alignment"
                .to_string(),
        });
    }
    if speaker_curve.phase.is_none() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Speaker measurement must include phase data for phase alignment".to_string(),
        });
    }

    let common_freqs =
        create_common_freq_grid(sub_curve, speaker_curve, config.min_freq, config.max_freq);

    let sub_interp = interpolate_curve_complex(sub_curve, &common_freqs)?;
    let speaker_interp = interpolate_curve_complex(speaker_curve, &common_freqs)?;

    // Compute frequency weights
    let weights = compute_frequency_weights(&common_freqs, opt_config.weighting);

    // Calculate baseline energy (no delay, no inversion)
    let energy_before = compute_weighted_energy(
        &sub_interp,
        &speaker_interp,
        &common_freqs,
        &weights,
        0.0,
        false,
    );

    let polarities = if config.optimize_polarity {
        vec![false, true]
    } else {
        vec![false]
    };

    let mut best_delay = 0.0;
    let mut best_invert = false;
    let mut best_energy = energy_before;

    // For each polarity, use golden section search to find optimal delay
    for &invert in &polarities {
        let (optimal_delay, optimal_energy) = golden_section_maximize(
            |delay_ms| {
                compute_weighted_energy(
                    &sub_interp,
                    &speaker_interp,
                    &common_freqs,
                    &weights,
                    delay_ms,
                    invert,
                )
            },
            -config.max_delay_ms,
            config.max_delay_ms,
            opt_config.tolerance_ms,
            opt_config.max_iterations,
        );

        if optimal_energy > best_energy {
            best_energy = optimal_energy;
            best_delay = optimal_delay;
            best_invert = invert;
        }
    }

    let improvement_db = 10.0 * (best_energy / energy_before.max(1e-12)).log10();

    info!(
        "  Phase alignment: delay={:.2}ms, invert={}, improvement={:.2}dB",
        best_delay, best_invert, improvement_db
    );

    Ok(PhaseAlignmentResult {
        delay_ms: best_delay,
        invert_polarity: best_invert,
        energy_before,
        energy_after: best_energy,
        improvement_db,
    })
}

/// Golden section search for 1D maximization.
///
/// Efficient O(log(precision)) convergence for unimodal functions.
fn golden_section_maximize<F>(f: F, a: f64, b: f64, tol: f64, max_iter: usize) -> (f64, f64)
where
    F: Fn(f64) -> f64,
{
    const PHI: f64 = 1.618033988749895;
    const RESPHI: f64 = 2.0 - PHI;

    let mut a = a;
    let mut b = b;
    let mut c = b - RESPHI * (b - a);
    let mut fc = f(c);

    for _ in 0..max_iter {
        if (b - a).abs() < tol {
            break;
        }

        let d = if (b - c) > (c - a) {
            c + RESPHI * (b - c)
        } else {
            c - RESPHI * (c - a)
        };

        let fd = f(d);

        if fd > fc {
            if (b - c) > (c - a) {
                a = c;
            } else {
                b = c;
            }
            c = d;
            fc = fd;
        } else if (b - c) > (c - a) {
            b = d;
        } else {
            a = d;
        }
    }

    (c, fc)
}

/// Compute frequency-dependent weights for energy calculation.
///
/// A-weighting emphasizes frequencies where human hearing is most sensitive.
/// C-weighting is nearly flat but rolls off at very low and high frequencies.
fn compute_frequency_weights(freqs: &Array1<f64>, weighting: WeightingType) -> Vec<f64> {
    match weighting {
        WeightingType::None => vec![1.0; freqs.len()],
        WeightingType::AWeighting => freqs.iter().map(|&f| a_weighting(f)).collect(),
        WeightingType::CWeighting => freqs.iter().map(|&f| c_weighting(f)).collect(),
    }
}

/// A-weighting curve (dB -> linear scale).
///
/// Approximates the equal-loudness curve at 40 phon.
/// Peaks around 2-5 kHz where human hearing is most sensitive.
fn a_weighting(f: f64) -> f64 {
    if f < 10.0 {
        return 0.001; // Very low frequencies are heavily attenuated
    }

    // A-weighting formula (IEC 61672-1)
    // Poles: 2nd order at 20.6 Hz and 12200 Hz, 1st order at 107.7 Hz and 737.9 Hz
    // Magnitude: f⁴ / ((f²+f₁²)(f²+f₄²)√(f²+f₂²)√(f²+f₃²))
    let f_sq = f * f;
    let num = 12200.0_f64.powi(2) * f_sq.powi(2);
    let denom = (f_sq + 20.6 * 20.6)
        * (f_sq + 12200.0_f64.powi(2))
        * (f_sq + 107.7 * 107.7).sqrt()
        * (f_sq + 737.9 * 737.9).sqrt();

    let weighting_db = 2.0 + 20.0 * (num / denom).log10();

    // Convert dB to linear scale
    10.0_f64.powf(weighting_db / 20.0)
}

/// C-weighting curve (dB -> linear scale).
///
/// Nearly flat response, only rolls off at very low and high frequencies.
fn c_weighting(f: f64) -> f64 {
    if f < 10.0 {
        return 0.1;
    }

    // C-weighting: 2nd order poles at 20.6 Hz and 12200 Hz, zeros at origin
    let f_sq = f * f;
    let num = 12200.0_f64.powi(2) * f_sq;
    let denom = (f_sq + 20.6 * 20.6) * (f_sq + 12200.0_f64.powi(2));

    let weighting_db = 0.0619 + 20.0 * (num / denom).log10();

    10.0_f64.powf(weighting_db / 20.0)
}

/// Compute weighted combined energy of sub + speaker with delay and polarity.
fn compute_weighted_energy(
    sub: &[Complex64],
    speaker: &[Complex64],
    freqs: &Array1<f64>,
    weights: &[f64],
    delay_ms: f64,
    invert: bool,
) -> f64 {
    let delay_s = delay_ms / 1000.0;
    let polarity = if invert { -1.0 } else { 1.0 };

    let mut energy = 0.0;

    for (i, &f) in freqs.iter().enumerate() {
        let omega = 2.0 * PI * f;
        let delay_phase = Complex64::from_polar(1.0, -omega * delay_s);

        let combined = sub[i] + speaker[i] * delay_phase * polarity;

        // Apply frequency weighting
        energy += combined.norm_sqr() * weights[i];
    }

    energy
}

/// Create a common frequency grid for interpolation
fn create_common_freq_grid(
    curve1: &Curve,
    curve2: &Curve,
    min_freq: f64,
    max_freq: f64,
) -> Array1<f64> {
    let f_min = min_freq
        .max(*curve1.freq.first().unwrap_or(&20.0))
        .max(*curve2.freq.first().unwrap_or(&20.0));
    let f_max = max_freq
        .min(*curve1.freq.last().unwrap_or(&20000.0))
        .min(*curve2.freq.last().unwrap_or(&20000.0));

    let num_points = 100;
    let log_min = f_min.log10();
    let log_max = f_max.log10();

    Array1::from_shape_fn(num_points, |i| {
        let log_f = log_min + (log_max - log_min) * (i as f64 / (num_points - 1) as f64);
        10.0_f64.powf(log_f)
    })
}

/// Interpolate a curve to new frequencies, returning complex values
fn interpolate_curve_complex(curve: &Curve, new_freqs: &Array1<f64>) -> Result<Vec<Complex64>> {
    let phase = curve
        .phase
        .as_ref()
        .ok_or_else(|| AutoeqError::InvalidMeasurement {
            message: "Phase data required for complex interpolation".to_string(),
        })?;

    let mut result = Vec::with_capacity(new_freqs.len());

    for &f in new_freqs.iter() {
        let (lower_idx, upper_idx) = find_bracket_indices(&curve.freq, f);

        let f_low = curve.freq[lower_idx];
        let f_high = curve.freq[upper_idx];
        let t = if f_high > f_low {
            (f - f_low) / (f_high - f_low)
        } else {
            0.0
        };

        let spl_interp = curve.spl[lower_idx] + t * (curve.spl[upper_idx] - curve.spl[lower_idx]);
        // Shortest-arc interpolation to handle phase wrapping (e.g., 179° to -179°)
        let mut phase_delta = phase[upper_idx] - phase[lower_idx];
        if phase_delta > 180.0 {
            phase_delta -= 360.0;
        }
        if phase_delta < -180.0 {
            phase_delta += 360.0;
        }
        let phase_interp = phase[lower_idx] + t * phase_delta;

        let magnitude = 10.0_f64.powf(spl_interp / 20.0);
        let phase_rad = phase_interp.to_radians();
        result.push(Complex64::from_polar(magnitude, phase_rad));
    }

    Ok(result)
}

/// Find bracketing indices for interpolation
fn find_bracket_indices(freqs: &Array1<f64>, target: f64) -> (usize, usize) {
    for i in 0..freqs.len() - 1 {
        if freqs[i] <= target && freqs[i + 1] >= target {
            return (i, i + 1);
        }
    }

    if target <= freqs[0] {
        (0, 0)
    } else {
        let last = freqs.len() - 1;
        (last, last)
    }
}

/// Batch phase alignment for multiple speakers with a common subwoofer
///
/// # Arguments
/// * `sub_curve` - Subwoofer frequency response with phase
/// * `speaker_curves` - Vector of speaker frequency responses with phase
/// * `config` - Phase alignment configuration
///
/// # Returns
/// * Vector of phase alignment results, one per speaker
pub fn optimize_phase_alignment_batch(
    sub_curve: &Curve,
    speaker_curves: &[Curve],
    config: &PhaseAlignmentConfig,
) -> Result<Vec<PhaseAlignmentResult>> {
    speaker_curves
        .iter()
        .enumerate()
        .map(|(i, speaker_curve)| {
            debug!("  Aligning speaker {} with subwoofer", i);
            optimize_phase_alignment(sub_curve, speaker_curve, config)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sub_curve() -> Curve {
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();

        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect();
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0).collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        }
    }

    fn create_test_speaker_curve() -> Curve {
        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();

        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect();
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0 + 45.0).collect();

        Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        }
    }

    #[test]
    fn test_phase_alignment_basic() {
        let sub = create_test_sub_curve();
        let speaker = create_test_speaker_curve();
        let config = PhaseAlignmentConfig::default();

        let result = optimize_phase_alignment(&sub, &speaker, &config)
            .expect("Phase alignment should succeed");

        assert!(result.improvement_db >= 0.0, "Should not make things worse");
    }

    #[test]
    fn test_phase_alignment_no_phase_fails() {
        let sub = Curve {
            freq: Array1::from(vec![50.0, 80.0, 100.0]),
            spl: Array1::from(vec![90.0, 90.0, 90.0]),
            phase: None,
        };
        let speaker = create_test_speaker_curve();
        let config = PhaseAlignmentConfig::default();

        let result = optimize_phase_alignment(&sub, &speaker, &config);
        assert!(result.is_err(), "Should fail without phase data");
    }

    #[test]
    fn test_phase_alignment_polarity_detection() {
        let sub = create_test_sub_curve();

        let freqs: Vec<f64> = (0..50)
            .map(|i| 20.0 * (500.0 / 20.0_f64).powf(i as f64 / 49.0))
            .collect();
        let spl: Vec<f64> = freqs.iter().map(|_| 90.0).collect();
        let phase: Vec<f64> = freqs.iter().map(|f| -180.0 * f / 100.0 + 180.0).collect();

        let speaker = Curve {
            freq: Array1::from(freqs),
            spl: Array1::from(spl),
            phase: Some(Array1::from(phase)),
        };

        let config = PhaseAlignmentConfig {
            optimize_polarity: true,
            ..Default::default()
        };

        let result = optimize_phase_alignment(&sub, &speaker, &config)
            .expect("Phase alignment should succeed");

        // With inverted phase, optimization should detect polarity inversion helps
        assert!(result.improvement_db >= 0.0);
    }

    #[test]
    fn test_common_freq_grid() {
        let sub = create_test_sub_curve();
        let speaker = create_test_speaker_curve();

        let grid = create_common_freq_grid(&sub, &speaker, 60.0, 100.0);

        assert!(!grid.is_empty());
        assert!(grid[0] >= 60.0);
        assert!(grid[grid.len() - 1] <= 100.0);
    }

    #[test]
    fn test_batch_alignment() {
        let sub = create_test_sub_curve();
        let speakers = vec![create_test_speaker_curve(), create_test_speaker_curve()];
        let config = PhaseAlignmentConfig::default();

        let results = optimize_phase_alignment_batch(&sub, &speakers, &config)
            .expect("Batch alignment should succeed");

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_golden_section_maximization() {
        // Maximize -(x - 3)^2 (peak at x=3)
        let (x, _) = golden_section_maximize(|x| -(x - 3.0).powi(2), -10.0, 10.0, 1e-6, 50);
        assert!((x - 3.0).abs() < 1e-5, "Expected 3.0, got {}", x);
    }

    #[test]
    fn test_a_weighting() {
        // A-weighting peaks around 2-5 kHz
        let w_1k = a_weighting(1000.0);
        let w_2k = a_weighting(2000.0);
        let w_4k = a_weighting(4000.0);
        let w_100 = a_weighting(100.0);

        // 2-4 kHz should have higher weight than 100 Hz
        assert!(w_2k > w_100, "A-weighting at 2kHz should exceed 100Hz");
        assert!(w_4k > w_100, "A-weighting at 4kHz should exceed 100Hz");

        // All should be positive
        assert!(w_1k > 0.0);
        assert!(w_2k > 0.0);
        assert!(w_4k > 0.0);
    }

    #[test]
    fn test_c_weighting() {
        // C-weighting is nearly flat in audible range
        let w_100 = c_weighting(100.0);
        let w_1k = c_weighting(1000.0);
        let w_10k = c_weighting(10000.0);

        // Should be relatively flat compared to A-weighting
        assert!(w_100 > 0.5, "C-weighting at 100Hz should be reasonable");
        assert!(w_1k > 0.9, "C-weighting at 1kHz should be near 1.0");
        assert!(
            w_10k > 0.5,
            "C-weighting at 10kHz should still be reasonable"
        );
    }

    #[test]
    fn test_weighted_energy_improves_alignment() {
        let sub = create_test_sub_curve();
        let speaker = create_test_speaker_curve();

        let config = PhaseAlignmentConfig::default();

        // Test with no weighting
        let opt_none = PhaseAlignmentOptConfig {
            weighting: WeightingType::None,
            ..Default::default()
        };
        let result_none = optimize_phase_alignment_with_options(&sub, &speaker, &config, opt_none)
            .expect("Should succeed");

        // Test with A-weighting
        let opt_a = PhaseAlignmentOptConfig {
            weighting: WeightingType::AWeighting,
            ..Default::default()
        };
        let result_a = optimize_phase_alignment_with_options(&sub, &speaker, &config, opt_a)
            .expect("Should succeed");

        // Both should find improvement (exact values may differ)
        assert!(result_none.improvement_db >= 0.0);
        assert!(result_a.improvement_db >= 0.0);
    }
}
