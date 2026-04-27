//! Spatial robustness analysis for multi-position room measurements.
//!
//! Implements spatial robustness optimization:
//! 1. RMS power spectrum averaging across measurement positions
//! 2. Spatial variance computation per frequency bin
//! 3. Correction depth mask: full correction where variance is low (room modes),
//!    reduced correction where variance is high (position-dependent effects)
//!
//! Reference: Brännmark & Sternad, AES 124th Convention (2008)
//! Reference: Patent EP2104374B1 — spatial zero clustering

use crate::Curve;
use crate::error::{AutoeqError, Result};
use ndarray::Array1;

/// Configuration for spatial robustness analysis.
#[derive(Debug, Clone)]
pub struct SpatialRobustnessConfig {
    /// Variance threshold (dB) below which full correction is allowed.
    /// Default: 3.0 dB
    pub variance_threshold_db: f64,

    /// Transition width (dB) for sigmoid blending between full and reduced correction.
    /// Default: 2.0 dB
    pub transition_width_db: f64,

    /// Minimum correction depth (0.0 to 1.0). Even high-variance frequencies get
    /// at least this much correction weight. Default: 0.1
    pub min_correction_depth: f64,

    /// Smoothing width in octaves for the correction depth mask.
    /// Prevents rapid mask changes between adjacent frequencies. Default: 1/6 octave.
    pub mask_smoothing_octaves: f64,
}

impl Default for SpatialRobustnessConfig {
    fn default() -> Self {
        Self {
            variance_threshold_db: 3.0,
            transition_width_db: 2.0,
            min_correction_depth: 0.1,
            mask_smoothing_octaves: 1.0 / 6.0,
        }
    }
}

/// Result of spatial robustness analysis.
#[derive(Debug, Clone)]
pub struct SpatialRobustnessResult {
    /// RMS-averaged frequency response across all positions.
    pub averaged_curve: Curve,

    /// Per-frequency standard deviation across positions (dB).
    pub spatial_variance: Array1<f64>,

    /// Per-frequency correction depth mask (0.0 = no correction, 1.0 = full correction).
    pub correction_depth: Array1<f64>,
}

/// Compute RMS power spectrum average across multiple measurement positions.
///
/// Unlike arithmetic averaging of dB values (which underweights loud positions)
/// or complex averaging (which causes phase cancellation), RMS averaging preserves
/// the energy content:
///
///   avg_spl[f] = 10 * log10(mean(10^(spl_i[f] / 10)))
///
/// All curves must share the same frequency axis.
pub fn rms_average(curves: &[Curve]) -> Curve {
    rms_average_weighted(curves, None)
}

pub fn rms_average_weighted(curves: &[Curve], weights: Option<&[f64]>) -> Curve {
    validate_spatial_curves(curves).expect("spatial robustness curves must be valid");
    let len = curves[0].freq.len();
    let weights = normalized_weights(curves.len(), weights);

    let mut avg_spl = Array1::zeros(len);
    for bin in 0..len {
        let sum_power: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * 10.0_f64.powf(c.spl[bin] / 10.0))
            .sum();
        avg_spl[bin] = 10.0 * sum_power.max(1e-12).log10();
    }

    Curve {
        freq: curves[0].freq.clone(),
        spl: avg_spl,
        phase: None,
        ..Default::default()
    }
}

/// Compute per-frequency standard deviation across positions (in dB).
///
/// A low std dev at a frequency means the feature is spatially consistent
/// (e.g., a room mode). A high std dev means position-dependent (e.g., comb
/// filtering from reflections arriving at different phase per position).
pub fn spatial_std_dev(curves: &[Curve]) -> Array1<f64> {
    spatial_std_dev_weighted(curves, None)
}

pub fn spatial_std_dev_weighted(curves: &[Curve], weights: Option<&[f64]>) -> Array1<f64> {
    validate_spatial_curves(curves).expect("spatial robustness curves must be valid");
    if curves.len() == 1 {
        // Single curve: zero variance everywhere
        return Array1::zeros(curves[0].freq.len());
    }
    let len = curves[0].freq.len();
    let weights = normalized_weights(curves.len(), weights);

    let mut std_dev = Array1::zeros(len);
    for bin in 0..len {
        let mean: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * c.spl[bin])
            .sum();
        let variance: f64 = curves
            .iter()
            .zip(weights.iter())
            .map(|(c, weight)| weight * (c.spl[bin] - mean).powi(2))
            .sum::<f64>();
        let unbiased_denominator = 1.0 - weights.iter().map(|w| w * w).sum::<f64>();
        let denominator_floor = 1.0 / curves.len() as f64;
        std_dev[bin] = (variance / unbiased_denominator.max(denominator_floor)).sqrt();
    }

    std_dev
}

/// Build a correction depth mask from spatial variance.
///
/// Uses a sigmoid function to transition smoothly between full correction
/// (where variance < threshold) and minimum correction (where variance >> threshold):
///
///   depth[f] = min_depth + (1 - min_depth) * sigmoid((threshold - var[f]) / width)
///
/// The mask is then smoothed in the log-frequency domain to avoid sharp transitions.
pub fn correction_depth_mask(
    freq: &Array1<f64>,
    spatial_variance: &Array1<f64>,
    config: &SpatialRobustnessConfig,
) -> Array1<f64> {
    let len = freq.len();
    let mut mask = Array1::zeros(len);

    // Sigmoid: maps (threshold - variance) / width through 1/(1+exp(-x))
    for i in 0..len {
        let sigmoid = if config.transition_width_db <= 0.0 {
            // Hard threshold: step function
            if spatial_variance[i] <= config.variance_threshold_db {
                1.0
            } else {
                0.0
            }
        } else {
            let x =
                (config.variance_threshold_db - spatial_variance[i]) / config.transition_width_db;
            1.0 / (1.0 + (-x).exp())
        };
        mask[i] = config.min_correction_depth + (1.0 - config.min_correction_depth) * sigmoid;
    }

    // Smooth the mask in log-frequency domain to prevent rapid oscillations
    if config.mask_smoothing_octaves > 0.0 {
        mask = smooth_log_frequency(&mask, freq, config.mask_smoothing_octaves);
    }

    mask
}

/// Perform full spatial robustness analysis on a set of multi-position measurements.
///
/// Returns the RMS-averaged curve, spatial variance, and correction depth mask.
pub fn analyze_spatial_robustness(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
) -> SpatialRobustnessResult {
    try_analyze_spatial_robustness_weighted(curves, config, None).unwrap_or_else(|e| panic!("{e}"))
}

pub fn analyze_spatial_robustness_weighted(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
    weights: Option<&[f64]>,
) -> SpatialRobustnessResult {
    try_analyze_spatial_robustness_weighted(curves, config, weights)
        .unwrap_or_else(|e| panic!("{e}"))
}

pub fn try_analyze_spatial_robustness_weighted(
    curves: &[Curve],
    config: &SpatialRobustnessConfig,
    weights: Option<&[f64]>,
) -> Result<SpatialRobustnessResult> {
    validate_spatial_curves(curves)?;

    let averaged_curve = rms_average_weighted(curves, weights);
    let spatial_variance = spatial_std_dev_weighted(curves, weights);
    let correction_depth = correction_depth_mask(&averaged_curve.freq, &spatial_variance, config);

    Ok(SpatialRobustnessResult {
        averaged_curve,
        spatial_variance,
        correction_depth,
    })
}

fn validate_spatial_curves(curves: &[Curve]) -> Result<()> {
    if curves.is_empty() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "spatial robustness needs at least one curve".to_string(),
        });
    }

    let reference = &curves[0].freq;
    if !is_valid_spatial_frequency_grid(reference) || curves[0].spl.len() != reference.len() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "spatial robustness reference curve has an invalid frequency grid".to_string(),
        });
    }

    for (idx, curve) in curves.iter().enumerate().skip(1) {
        if !is_valid_spatial_frequency_grid(&curve.freq) || curve.spl.len() != curve.freq.len() {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "spatial robustness curve {} has an invalid frequency grid",
                    idx
                ),
            });
        }
        if !super::frequency_grid::same_frequency_grid(reference, &curve.freq) {
            return Err(AutoeqError::InvalidMeasurement {
                message: format!(
                    "spatial robustness curves must share the same frequency grid; curve {} differs",
                    idx
                ),
            });
        }
    }

    Ok(())
}

fn is_valid_spatial_frequency_grid(freq: &Array1<f64>) -> bool {
    !freq.is_empty()
        && freq.iter().all(|f| f.is_finite() && *f > 0.0)
        && freq.windows(2).into_iter().all(|pair| pair[0] < pair[1])
}

fn normalized_weights(len: usize, weights: Option<&[f64]>) -> Vec<f64> {
    let Some(weights) = weights else {
        return vec![1.0 / len as f64; len];
    };
    if weights.len() != len {
        return vec![1.0 / len as f64; len];
    }
    let mut clean: Vec<f64> = weights
        .iter()
        .map(|w| if w.is_finite() && *w > 0.0 { *w } else { 0.0 })
        .collect();
    let sum: f64 = clean.iter().sum();
    if sum <= f64::EPSILON {
        return vec![1.0 / len as f64; len];
    }
    for weight in &mut clean {
        *weight /= sum;
    }
    clean
}

/// Smooth an array using a sliding window in log-frequency domain.
///
/// Window width is specified in octaves. Each output sample is the average
/// of all input samples within +/- half_width octaves.
fn smooth_log_frequency(data: &Array1<f64>, freq: &Array1<f64>, width_octaves: f64) -> Array1<f64> {
    let len = data.len();
    let half_width = width_octaves / 2.0;
    let mut smoothed = Array1::zeros(len);

    if !freq.windows(2).into_iter().all(|pair| pair[0] <= pair[1]) {
        for i in 0..len {
            let center_log = freq[i].log2();
            let low_log = center_log - half_width;
            let high_log = center_log + half_width;

            let mut sum = 0.0;
            let mut count = 0.0;
            for j in 0..len {
                let f_log = freq[j].log2();
                if f_log >= low_log && f_log <= high_log {
                    sum += data[j];
                    count += 1.0;
                }
            }

            smoothed[i] = if count > 0.0 { sum / count } else { data[i] };
        }
        return smoothed;
    }

    let logs: Vec<f64> = freq.iter().map(|f| f.log2()).collect();
    let mut left = 0usize;
    let mut right = 0usize;
    let mut sum = 0.0;

    for i in 0..len {
        let low_log = logs[i] - half_width;
        let high_log = logs[i] + half_width;

        while right < len && logs[right] <= high_log {
            sum += data[right];
            right += 1;
        }

        while left < right && logs[left] < low_log {
            sum -= data[left];
            left += 1;
        }

        let count = right - left;
        smoothed[i] = if count > 0 {
            sum / count as f64
        } else {
            data[i]
        };
    }

    smoothed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_curve(freq: Vec<f64>, spl: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freq),
            spl: Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_rms_average_identical_curves() {
        let curve = make_curve(vec![100.0, 1000.0, 10000.0], vec![80.0, 85.0, 75.0]);
        let avg = rms_average(&[curve.clone(), curve.clone()]);

        // RMS average of identical curves should equal the original
        for i in 0..3 {
            assert!(
                (avg.spl[i] - curve.spl[i]).abs() < 0.01,
                "bin {}: expected {}, got {}",
                i,
                curve.spl[i],
                avg.spl[i]
            );
        }
    }

    #[test]
    fn test_rms_average_vs_arithmetic() {
        // RMS average should be higher than arithmetic mean of dB values
        // because averaging in power domain weights louder values more
        let c1 = make_curve(vec![100.0], vec![80.0]);
        let c2 = make_curve(vec![100.0], vec![90.0]);
        let avg = rms_average(&[c1, c2]);

        let arithmetic_mean = (80.0 + 90.0) / 2.0; // = 85.0
        assert!(
            avg.spl[0] > arithmetic_mean,
            "RMS average ({:.2}) should be > arithmetic mean ({:.2})",
            avg.spl[0],
            arithmetic_mean
        );
    }

    #[test]
    fn test_spatial_std_dev_identical() {
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let std = spatial_std_dev(&[curve.clone(), curve.clone()]);
        assert!(std[0] < 0.01);
        assert!(std[1] < 0.01);
    }

    #[test]
    fn test_spatial_std_dev_different() {
        let c1 = make_curve(vec![100.0], vec![80.0]);
        let c2 = make_curve(vec![100.0], vec![86.0]);
        let std = spatial_std_dev(&[c1, c2]);

        // std_dev of [80, 86] = sqrt(((80-83)^2 + (86-83)^2) / 1) = sqrt(18) ≈ 4.24
        assert!(
            (std[0] - 4.24).abs() < 0.1,
            "expected ~4.24, got {}",
            std[0]
        );
    }

    #[test]
    fn test_spatial_std_dev_skewed_weights_do_not_zero_variance() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 80.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![100.0, 100.0]);
        let c3 = make_curve(vec![100.0, 1000.0], vec![100.0, 100.0]);
        let std = spatial_std_dev_weighted(&[c1, c2, c3], Some(&[1.0, 1e-18, 1e-18]));

        assert!(
            std[0] > 0.0 && std[0].is_finite(),
            "skewed non-zero weights should not collapse variance to zero, got {}",
            std[0]
        );
    }

    #[test]
    #[should_panic(expected = "invalid frequency grid")]
    fn test_spatial_std_dev_rejects_mismatched_spl_lengths() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![80.0]);
        let _ = spatial_std_dev(&[c1, c2]);
    }

    #[test]
    #[should_panic(expected = "invalid frequency grid")]
    fn test_rms_average_rejects_mismatched_spl_lengths() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![100.0, 1000.0], vec![80.0]);
        let _ = rms_average(&[c1, c2]);
    }

    #[test]
    fn test_correction_depth_low_variance() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![0.5]); // well below threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0, // disable smoothing for test
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        // sigmoid((3.0 - 0.5) / 2.0) = sigmoid(1.25) ≈ 0.777
        // depth = 0.1 + 0.9 * 0.777 ≈ 0.80
        assert!(
            depth[0] > 0.75,
            "low variance should give high correction, got {}",
            depth[0]
        );
    }

    #[test]
    fn test_correction_depth_high_variance() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![10.0]); // well above threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        assert!(
            depth[0] < 0.3,
            "high variance should give reduced correction, got {}",
            depth[0]
        );
        assert!(
            depth[0] >= config.min_correction_depth,
            "should never go below min_correction_depth"
        );
    }

    #[test]
    fn test_correction_depth_at_threshold() {
        let freq = Array1::from_vec(vec![100.0]);
        let variance = Array1::from_vec(vec![3.0]); // exactly at threshold
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        // At threshold, sigmoid(0) = 0.5, so depth = min + (1-min)*0.5
        let expected = 0.1 + 0.9 * 0.5;
        assert!(
            (depth[0] - expected).abs() < 0.01,
            "expected ~{:.2}, got {:.2}",
            expected,
            depth[0]
        );
    }

    #[test]
    fn test_correction_depth_zero_transition_width() {
        // Bug fix: transition_width_db = 0.0 should use hard threshold (not divide by zero)
        let freq = Array1::from_vec(vec![100.0, 200.0]);
        let variance = Array1::from_vec(vec![1.0, 5.0]); // below and above threshold
        let config = SpatialRobustnessConfig {
            variance_threshold_db: 3.0,
            transition_width_db: 0.0, // hard threshold
            min_correction_depth: 0.1,
            mask_smoothing_octaves: 0.0,
        };

        let depth = correction_depth_mask(&freq, &variance, &config);

        // Below threshold → full correction
        assert!(
            depth[0] > 0.9,
            "below threshold should give full correction, got {}",
            depth[0]
        );
        // Above threshold → min correction
        assert!(
            (depth[1] - 0.1).abs() < 0.01,
            "above threshold should give min correction, got {}",
            depth[1]
        );
    }

    #[test]
    fn test_spatial_std_dev_single_curve() {
        // Bug fix: single curve should return zero variance (not panic)
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let std = spatial_std_dev(&[curve]);
        assert_eq!(std[0], 0.0);
        assert_eq!(std[1], 0.0);
    }

    #[test]
    fn test_analyze_spatial_robustness_single_curve() {
        // Bug fix: single-curve analysis should work (not panic in spatial_std_dev)
        let curve = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let result = analyze_spatial_robustness(&[curve], &config);

        // Zero variance → high correction everywhere
        // sigmoid((3.0 - 0.0) / 2.0) ≈ 0.818, depth = 0.1 + 0.9 * 0.818 ≈ 0.836
        assert!(result.spatial_variance.iter().all(|&v| v == 0.0));
        assert!(
            result.correction_depth.iter().all(|&d| d > 0.8),
            "single curve should have high correction depth, got min={:.3}",
            result
                .correction_depth
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min)
        );
    }

    #[test]
    #[should_panic(expected = "same frequency grid")]
    fn test_analyze_spatial_robustness_rejects_mismatched_frequency_grids() {
        let c1 = make_curve(vec![100.0, 1000.0], vec![80.0, 85.0]);
        let c2 = make_curve(vec![110.0, 1100.0], vec![80.0, 85.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };

        let _ = analyze_spatial_robustness(&[c1, c2], &config);
    }

    #[test]
    fn test_full_analysis() {
        // Room mode at 100 Hz (consistent), comb filter at 5 kHz (inconsistent)
        let c1 = make_curve(vec![100.0, 5000.0], vec![90.0, 80.0]);
        let c2 = make_curve(vec![100.0, 5000.0], vec![91.0, 72.0]);
        let c3 = make_curve(vec![100.0, 5000.0], vec![89.0, 85.0]);

        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.0,
            ..Default::default()
        };
        let result = analyze_spatial_robustness(&[c1, c2, c3], &config);

        // 100 Hz should have low variance → high correction depth
        assert!(result.spatial_variance[0] < 2.0);
        assert!(result.correction_depth[0] > 0.7);

        // 5 kHz should have high variance → low correction depth
        assert!(result.spatial_variance[1] > 5.0);
        assert!(result.correction_depth[1] < 0.5);
    }

    #[test]
    fn test_rms_average_negative_spl() {
        // Negative SPL values (relative measurements) should work correctly
        let c1 = make_curve(vec![100.0], vec![-10.0]);
        let c2 = make_curve(vec![100.0], vec![-20.0]);
        let avg = rms_average(&[c1, c2]);
        // RMS average in power domain: 10*log10(mean(10^(-10/10), 10^(-20/10)))
        // = 10*log10(mean(0.1, 0.01)) = 10*log10(0.055) ≈ -12.6 dB
        assert!(avg.spl[0] > -20.0 && avg.spl[0] < -10.0);
        assert!(avg.spl[0].is_finite());
    }

    #[test]
    fn test_smooth_log_frequency_reduces_variation() {
        // Smoothing should reduce rapid oscillations
        // Use wider frequency span so the window doesn't cover everything
        let freq = Array1::from_vec(vec![
            50.0, 70.0, 100.0, 140.0, 200.0, 280.0, 400.0, 560.0, 800.0, 1120.0, 1600.0,
        ]);
        let data = Array1::from_vec(vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);

        let smoothed = smooth_log_frequency(&data, &freq, 1.5); // 1.5 octave window

        // Smoothed should have less variation than original
        let orig_range = 1.0;
        let smooth_range = smoothed.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - smoothed.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            smooth_range < orig_range,
            "smoothing should reduce range: orig={:.2}, smoothed={:.2}",
            orig_range,
            smooth_range
        );
    }

    #[test]
    fn test_smooth_log_frequency_preserves_constant() {
        // Smoothing a constant array should return the same constant
        let freq = Array1::from_vec(vec![100.0, 200.0, 400.0, 800.0]);
        let data = Array1::from_vec(vec![0.5, 0.5, 0.5, 0.5]);
        let smoothed = smooth_log_frequency(&data, &freq, 1.0);
        for &v in smoothed.iter() {
            assert!((v - 0.5).abs() < 0.001);
        }
    }

    #[test]
    fn test_correction_depth_with_smoothing_enabled() {
        // Test that smoothing doesn't produce NaN or out-of-range values
        let freq = Array1::from_vec(vec![50.0, 100.0, 200.0, 500.0, 1000.0]);
        let variance = Array1::from_vec(vec![1.0, 8.0, 1.0, 8.0, 1.0]);
        let config = SpatialRobustnessConfig {
            mask_smoothing_octaves: 0.5, // smoothing enabled
            ..Default::default()
        };

        let depth = correction_depth_mask(&freq, &variance, &config);
        for &d in depth.iter() {
            assert!(d.is_finite(), "depth should be finite");
            assert!(
                (0.0..=1.0).contains(&d),
                "depth should be in [0, 1], got {}",
                d
            );
        }
    }
}
